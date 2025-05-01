use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use clockabilly::prelude::*;
use json_patch_ext::prelude::*;
use kube::core::admission::{
    AdmissionRequest,
    AdmissionResponse,
    AdmissionReview,
};
use rocket::serde::json::Json;
use serde_json::{
    json,
    Value,
};
use sk_core::jsonutils;
use sk_core::k8s::{
    PodExt,
    PodLifecycleData,
    GVK,
};
use sk_core::prelude::*;
use tracing::*;

use crate::util::compute_step_size;
use crate::DriverContext;

pub struct MutationData {
    pod_counts: Mutex<HashMap<u64, usize>>,
}

impl MutationData {
    pub fn new() -> MutationData {
        MutationData { pod_counts: Mutex::new(HashMap::new()) }
    }

    pub fn count(&self, hash: u64) -> usize {
        let mut pod_counts = self.pod_counts.lock().unwrap();
        *pod_counts.entry(hash).and_modify(|e| *e += 1).or_default()
    }
}

#[rocket::post("/", data = "<body>")]
pub async fn handler(
    ctx: &rocket::State<DriverContext>,
    sim: &rocket::State<Simulation>,
    body: Json<AdmissionReview<corev1::Pod>>,
    mut_data: &rocket::State<MutationData>,
) -> Json<AdmissionReview<corev1::Pod>> {
    let req: AdmissionRequest<_> = match body.into_inner().try_into() {
        Ok(r) => r,
        Err(err) => {
            error!("could not parse request: {err:?}");
            let resp = AdmissionResponse::invalid(err);
            return Json(into_pod_review(resp));
        },
    };

    let mut resp = AdmissionResponse::from(&req);
    if let Some(pod) = &req.object {
        resp = mutate_pod(ctx, sim, resp, pod, mut_data, UtcClock::boxed())
            .await
            .unwrap_or_else(|err| {
                error!("could not perform mutation, blocking pod object: {err:?}");
                AdmissionResponse::from(&req).deny(err)
            });
    }

    Json(into_pod_review(resp))
}

// the instrument wrapper seems to mess with the coverage data
#[instrument(skip_all, fields(pod.namespaced_name=pod.namespaced_name()))]
pub async fn mutate_pod(
    ctx: &DriverContext,
    sim: &Simulation,
    resp: AdmissionResponse,
    // TODO when we get the pod object, the final name hasn't been filled in yet;
    // make sure this doesn't cause any problems
    pod: &corev1::Pod,
    mut_data: &MutationData,
    clock: Box<dyn Clockable + Send>,
) -> anyhow::Result<AdmissionResponse> {
    // enclose in a block so we release the mutex when we're done
    let owners = {
        let mut owners_cache = ctx.owners_cache.lock().await;
        owners_cache.compute_owner_chain(pod).await?
    };

    if !owners.iter().any(|o| o.name == ctx.root_name) {
        debug!("pod not owned by simulation, no mutation performed");
        return Ok(resp);
    }

    let hash = match pod.annotations().get(POD_SPEC_STABLE_HASH_KEY) {
        Some(hash_str) => hash_str.parse::<u64>()?,
        None => jsonutils::hash(&serde_json::to_value(&pod.stable_spec()?)?),
    };
    let seq = match pod.annotations().get(POD_SEQUENCE_NUMBER_KEY) {
        Some(seq_str) => seq_str.parse::<usize>()?,
        None => mut_data.count(hash),
    };
    info!("mutating pod (hash={hash}, seq={seq})");

    let mut patches = vec![];
    add_empty_labels_annotations(pod, &mut patches);
    if !pod.labels_contains_key(SIMULATION_LABEL_KEY) {
        info!("first time seeing pod, adding tracking annotations");
        patches.push(add_operation(format_ptr!("/metadata/labels/{}", escape(SIMULATION_LABEL_KEY)), json!(ctx.name)));
        add_node_selector_tolerations(pod, &mut patches)?;
        add_pod_hash_annotations(hash, seq, &mut patches);
    }
    if matches!(pod.status.as_ref(), Some(corev1::PodStatus{phase: Some(phase), ..}) if phase == "Running")
        && !pod.labels_contains_key(KWOK_STAGE_COMPLETE_KEY)
    {
        add_lifecycle_fields(ctx, sim, pod, &owners, hash, seq, &mut patches, clock)?;
    }

    // We can't use json_patch_ext stuff here because the AdmissionResponse is a part of Kubernetes
    // and doesn't know anything at all about our custom json_patch extensions
    Ok(resp.with_patch(Patch(patches))?)
}

fn add_empty_labels_annotations(pod: &corev1::Pod, patches: &mut Vec<PatchOperation>) {
    if pod.metadata.labels.is_none() {
        patches.push(add_operation(format_ptr!("/metadata/labels"), json!({})));
    }
    if pod.metadata.annotations.is_none() {
        patches.push(add_operation(format_ptr!("/metadata/annotations"), json!({})));
    }
}

fn add_node_selector_tolerations(pod: &corev1::Pod, patches: &mut Vec<PatchOperation>) -> EmptyResult {
    if pod.spec()?.tolerations.is_none() {
        patches.push(add_operation(format_ptr!("/spec/tolerations"), json!([])));
    }
    patches.push(add_operation(format_ptr!("/spec/nodeSelector"), json!({"type": "virtual"})));
    patches.push(add_operation(
        format_ptr!("/spec/tolerations/-"),
        json!({"key": VIRTUAL_NODE_TOLERATION_KEY, "operator": "Exists", "effect": "NoSchedule"}),
    ));

    Ok(())
}

fn add_pod_hash_annotations(hash: u64, seq: usize, patches: &mut Vec<PatchOperation>) {
    patches.push(add_operation(
        format_ptr!("/metadata/annotations/{}", escape(POD_SPEC_STABLE_HASH_KEY)),
        json!(format!("{hash}")),
    ));
    patches.push(add_operation(
        format_ptr!("/metadata/annotations/{}", escape(POD_SEQUENCE_NUMBER_KEY)),
        json!(format!("{seq}")),
    ));
}

#[allow(clippy::too_many_arguments)]
fn add_lifecycle_fields(
    ctx: &DriverContext,
    sim: &Simulation,
    pod: &corev1::Pod,
    owners: &[metav1::OwnerReference],
    hash: u64,
    seq: usize,
    patches: &mut Vec<PatchOperation>,
    clock: Box<dyn Clockable + Send>,
) -> EmptyResult {
    if let Some(orig_ns) = pod.annotations().get(ORIG_NAMESPACE_ANNOTATION_KEY) {
        for owner in owners {
            let owner_gvk = GVK::from_owner_ref(owner)?;
            let owner_ns_name = format!("{}/{}", orig_ns, owner.name);
            if !ctx.store.has_obj(&owner_gvk, &owner_ns_name) {
                continue;
            }
            let lifecycle = ctx.store.lookup_pod_lifecycle(&owner_gvk, &owner_ns_name, hash, seq);
            if let Some(patch) = to_completion_time_annotation(sim.speed(), &lifecycle, &*clock) {
                info!("applying lifecycle annotations");
                patches.push(add_operation(
                    format_ptr!("/metadata/labels/{}", escape(KWOK_STAGE_COMPLETE_KEY)),
                    json!("true"),
                ));
                patches.push(patch);
                break;
            } else {
                warn!("no pod lifecycle data found");
            }
        }
    }

    Ok(())
}

fn to_completion_time_annotation(
    speed: f64,
    pld: &PodLifecycleData,
    clock: &(dyn Clockable + Send),
) -> Option<PatchOperation> {
    match pld {
        PodLifecycleData::Empty | PodLifecycleData::Running(_) => None,
        PodLifecycleData::Finished(start_ts, end_ts) => {
            // Pause time doesn't factor into pod lifecycle times, so just set to 0 here
            let duration_secs = compute_step_size(speed, *start_ts, *end_ts);
            let duration = Duration::from_secs(duration_secs as u64);
            let abs_end_ts = clock.now() + duration;
            Some(add_operation(
                format_ptr!("/metadata/annotations/{}", escape(KWOK_STAGE_COMPLETE_TIMESTAMP_KEY)),
                Value::String(abs_end_ts.to_rfc3339()),
            ))
        },
    }
}

// Have to duplicate this fn because AdmissionResponse::into_review uses the dynamic API
fn into_pod_review(resp: AdmissionResponse) -> AdmissionReview<corev1::Pod> {
    AdmissionReview {
        types: resp.types.clone(),
        // All that matters is that we keep the request UUID, which is in the TypeMeta
        request: None,
        response: Some(resp),
    }
}
