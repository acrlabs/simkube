use std::collections::HashMap;
use std::iter;
use std::sync::{
    LazyLock,
    Mutex,
};
use std::time::Duration;

use clockabilly::prelude::*;
use json_patch_ext::prelude::*;
use kube::core::admission::{
    AdmissionRequest,
    AdmissionResponse,
    AdmissionReview,
    Operation as AdmissionOperation,
};
use regex::Regex;
use rocket::serde::json::Json;
use serde_json::{
    Value,
    json,
};
use sk_core::jsonutils;
use sk_core::k8s::{
    GVK,
    PodExt,
    PodLifecycleData,
    build_pod_self_owner_reference,
    pod_is_running,
    sanitize_obj,
};
use sk_core::prelude::*;
use tracing::*;

use crate::DriverContext;
use crate::util::compute_step_size;

static RESCHEDULE_COUNTER_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(.*)-clone-(\d+)$").unwrap());

pub struct MutationData {
    pod_counts: Mutex<HashMap<u64, usize>>,
    clock: Box<dyn Clockable + Send + Sync>,
}

impl MutationData {
    pub fn new() -> MutationData {
        MutationData {
            pod_counts: Mutex::new(HashMap::new()),
            clock: UtcClock::boxed(),
        }
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
    info!(
        "incoming {:?} request: object = {:?}, old_object = {:?}",
        req.operation,
        req.object.as_ref().map(|o| o.name_any()),
        req.old_object.as_ref().map(|o| o.name_any()),
    );
    debug!("full request: {req:?}");

    let mut resp = AdmissionResponse::from(&req);
    let pod = match req.operation {
        // object "should" always be populated for create/update, and old_object "should" always be
        // populated for delete, so these unwraps "should" be safe
        AdmissionOperation::Create | AdmissionOperation::Update => req.object.as_ref().unwrap(),
        AdmissionOperation::Delete => req.old_object.as_ref().unwrap(),
        _ => unreachable!("webhook only accepts create, update, or delete operations"),
    };

    // enclose in a block so we release the mutex when we're done
    let owners = {
        let mut owners_cache = ctx.owners_cache.lock().await;
        owners_cache.compute_owners_for(&corev1::Pod::gvk(), pod).await
    };
    if !owners.iter().any(|o| o.name == ctx.root_name) {
        debug!("pod not owned by simulation, no mutation performed");
        return Json(into_pod_review(resp));
    }

    let filtered_owners = owners
        .into_iter()
        .filter(|o| !o.api_version.starts_with(SIMKUBE_IO_PREFIX))
        .collect::<Vec<_>>();

    // No need to recheck if `reschedule_interrupted_bare_pods` is set here because the webhook is
    // only configured to receive DELETEs if that parameter is true
    if req.operation == AdmissionOperation::Delete {
        reschedule_interrupted_pod(ctx, &filtered_owners, pod)
            .await
            .unwrap_or_else(|err| {
                error!("could not reschedule pod; allowing old pod to be deleted: {err}");
            });
    } else if pod.metadata.deletion_timestamp.is_none() {
        resp = mutate_pod(ctx, sim, &req.operation, &filtered_owners, pod, resp, mut_data)
            .await
            .unwrap_or_else(|err| {
                // It's important to block the pod here if the mutation fails, otherwise all kinds
                // of havoc will occur if a pod meant for a simulated node gets scheduled on a real one
                error!("could not perform mutation, blocking pod object: {err}");
                AdmissionResponse::from(&req).deny(err)
            });
    }

    Json(into_pod_review(resp))
}

// the instrument wrapper seems to mess with the coverage data
#[instrument(skip_all, fields(pod.namespaced_name=pod.namespaced_name()))]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn mutate_pod(
    ctx: &DriverContext,
    sim: &Simulation,
    // BE AWARE: this gets called on both CREATE and UPDATE events, which means
    // this function should not change any read-only fields on UPDATE.
    op: &AdmissionOperation,
    owners: &[metav1::OwnerReference],
    pod: &corev1::Pod,
    resp: AdmissionResponse,
    mut_data: &MutationData,
) -> anyhow::Result<AdmissionResponse> {
    let (hash, seq) = lookup_pod_hash_sequence(pod, mut_data)?;

    let mut patches = vec![];
    add_empty_labels_annotations(pod, &mut patches);
    if *op == AdmissionOperation::Create {
        info!("first time seeing pod, adding tracking annotations");
        patches
            .push(add_operation(format_ptr!("/metadata/labels/{}", escape(SIMULATION_LABEL_KEY)), json!(ctx.sim_name)));
        add_node_selector_tolerations(pod, &mut patches)?;
        add_pod_hash_annotations(hash, seq, &mut patches);
        add_delay_annotations(sim, &mut patches);
    }

    if pod_is_running(pod) && !pod.labels_contains_key(KWOK_STAGE_COMPLETE_KEY) {
        info!("adding lifecycle annotations for pod (hash={hash}, seq={seq})");
        add_lifecycle_fields(ctx, sim, pod, owners, hash, seq, &mut patches, &*mut_data.clock)?;
    }

    // We can't use json_patch_ext stuff here because the AdmissionResponse is a part of Kubernetes
    // and doesn't know anything at all about our custom json_patch extensions
    Ok(resp.with_patch(Patch(patches))?)
}

#[instrument(skip_all, fields(pod.namespaced_name=pod.namespaced_name()))]
pub(crate) async fn reschedule_interrupted_pod(
    ctx: &DriverContext,
    filtered_owners: &[metav1::OwnerReference],
    pod: &corev1::Pod,
) -> EmptyResult {
    debug!("{filtered_owners:?}");
    if filtered_owners.is_empty() && pod_is_running(pod) {
        info!("detected bare pod {} interrupted before termination, rescheduling", pod.namespaced_name());

        let mut new_pod = pod.clone();
        let orig_pod_name = new_pod.name_any();
        let pod_namespace = pod.metadata.namespace.as_ref().unwrap();
        let new_pod_name = build_rescheduled_pod_name(&orig_pod_name);
        new_pod.metadata.name = Some(new_pod_name);

        sanitize_obj(&mut new_pod);
        new_pod.annotations_mut().retain(|k, _| !(k.starts_with(SIMKUBE_IO_PREFIX)));
        new_pod.labels_mut().retain(|k, _| !(k.starts_with(SIMKUBE_IO_PREFIX)));
        new_pod.spec.get_or_insert_default().node_name = None;
        new_pod.status = None;

        new_pod
            .annotations_mut()
            .entry(ORIG_OWNER_ANNOTATION_KEY.into())
            .or_insert(orig_pod_name);

        debug!("resubmitting pod: {new_pod:?}");
        let pod_api: kube::Api<corev1::Pod> = kube::Api::namespaced(ctx.client.clone(), pod_namespace);
        pod_api.create(&Default::default(), &new_pod).await?;
    }

    Ok(())
}

fn add_empty_labels_annotations(pod: &corev1::Pod, patches: &mut Vec<PatchOperation>) {
    if pod.metadata.labels.is_none() {
        patches.push(add_operation(format_ptr!("/metadata/labels"), json!({})));
    }
    if pod.metadata.annotations.is_none() {
        patches.push(add_operation(format_ptr!("/metadata/annotations"), json!({})));
    }
}

pub(crate) fn add_node_selector_tolerations(pod: &corev1::Pod, patches: &mut Vec<PatchOperation>) -> EmptyResult {
    if pod.spec()?.tolerations.is_none() {
        patches.push(add_operation(format_ptr!("/spec/tolerations"), json!([])));
    }
    if pod.spec()?.node_selector.is_none() {
        patches.push(add_operation(format_ptr!("/spec/nodeSelector"), json!({})));
    }
    // This will overwrite if there is a different nodeSelector with the key `type`
    // TODO (SK-276) we should someday turn this into `simkube.io/node-type`;
    patches.push(add_operation(format_ptr!("/spec/nodeSelector/type"), json!("virtual")));

    if pod
        .spec()?
        .tolerations
        .as_ref()
        .is_none_or(|tolerations| tolerations.iter().all(|t| t.key != Some(VIRTUAL_NODE_TOLERATION_KEY.into())))
    {
        patches.push(add_operation(
            format_ptr!("/spec/tolerations/-"),
            json!({"key": VIRTUAL_NODE_TOLERATION_KEY, "operator": "Exists", "effect": "NoSchedule"}),
        ));
    }

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

fn add_delay_annotations(sim: &Simulation, patches: &mut Vec<PatchOperation>) {
    patches.push(add_operation(
        format_ptr!("/metadata/annotations/{}", escape(KWOK_STAGE_CREATE_DELAY_KEY)),
        json!(format!("{}ms", sim.spec.lifecycle_params.image_pull_delay.unwrap_or(0).to_string())),
    ));
    patches.push(add_operation(
        format_ptr!("/metadata/annotations/{}", escape(KWOK_STAGE_CREATE_DELAY_JITTER_KEY)),
        json!(format!("{}ms", sim.spec.lifecycle_params.image_pull_jitter.unwrap_or(0).to_string())),
    ));
    patches.push(add_operation(
        format_ptr!("/metadata/annotations/{}", escape(KWOK_STAGE_READY_DELAY_KEY)),
        json!(format!("{}ms", sim.spec.lifecycle_params.pod_startup_delay.unwrap_or(0).to_string())),
    ));
    patches.push(add_operation(
        format_ptr!("/metadata/annotations/{}", escape(KWOK_STAGE_READY_DELAY_JITTER_KEY)),
        json!(format!("{}ms", sim.spec.lifecycle_params.pod_startup_jitter.unwrap_or(0).to_string())),
    ));
}

#[allow(clippy::too_many_arguments)]
fn add_lifecycle_fields(
    ctx: &DriverContext,
    sim: &Simulation,
    pod: &corev1::Pod,
    filtered_owners: &[metav1::OwnerReference],
    hash: u64,
    seq: usize,
    patches: &mut Vec<PatchOperation>,
    clock: &(dyn Clockable + Send),
) -> EmptyResult {
    if let Some(orig_ns) = pod.annotations().get(ORIG_NAMESPACE_ANNOTATION_KEY) {
        // To handle the "bare pod" case, we also look to see if the pod has been recorded as its
        // own owner in the trace file, or via the ORIG_OWNER_ANNOTATION_KEY
        let self_owner_name = pod
            .annotations()
            .get(ORIG_OWNER_ANNOTATION_KEY)
            .cloned()
            .unwrap_or(pod.name_any());
        for owner in filtered_owners
            .iter()
            .chain(iter::once(&build_pod_self_owner_reference(self_owner_name)))
        {
            let owner_gvk = GVK::from_owner_ref(owner)?;
            let owner_ns_name = format!("{}/{}", orig_ns, owner.name);
            if !ctx.trace.has_obj(&owner_gvk, &owner_ns_name) {
                debug!("owner {owner_gvk}.{owner_ns_name} for {} not found in trace", pod.namespaced_name());
                continue;
            }
            let lifecycle = ctx.trace.lookup_pod_lifecycle(&owner_gvk, &owner_ns_name, hash, seq);
            if let Some(patch) = to_completion_time_annotation(sim.speed(), &lifecycle, clock) {
                info!("applying lifecycle labels and annotations");
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

fn build_rescheduled_pod_name(orig_pod_name: &str) -> String {
    RESCHEDULE_COUNTER_REGEX
        .captures(orig_pod_name)
        .map_or(format!("{}-clone-1", orig_pod_name), |c| {
            let base = c.get(1).unwrap().as_str();
            let counter = c.get(2).unwrap().as_str().parse::<usize>().unwrap();
            format!("{base}-clone-{}", counter + 1)
        })
}

fn lookup_pod_hash_sequence(pod: &corev1::Pod, mut_data: &MutationData) -> anyhow::Result<(u64, usize)> {
    let hash = match pod.annotations().get(POD_SPEC_STABLE_HASH_KEY) {
        Some(hash_str) => hash_str.parse::<u64>()?,
        None => {
            let stable_spec = pod.stable_spec()?;
            debug!("computing pod stable spec: {:?}", stable_spec);
            jsonutils::hash(&serde_json::to_value(&stable_spec)?)
        },
    };
    let seq = match pod.annotations().get(POD_SEQUENCE_NUMBER_KEY) {
        Some(seq_str) => seq_str.parse::<usize>()?,
        None => mut_data.count(hash),
    };

    Ok((hash, seq))
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

#[cfg(test)]
impl MutationData {
    pub(crate) fn new_from_parts(
        pod_counts: Mutex<HashMap<u64, usize>>,
        clock: Box<dyn Clockable + Send + Sync>,
    ) -> MutationData {
        MutationData { pod_counts, clock }
    }
}
