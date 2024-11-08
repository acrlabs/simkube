use std::collections::HashMap;
use std::sync::Mutex;

use json_patch::{
    AddOperation,
    Patch,
    PatchOperation,
};
use json_patch_ext::escape;
use kube::core::admission::{
    AdmissionRequest,
    AdmissionResponse,
    AdmissionReview,
};
use kube::ResourceExt;
use rocket::serde::json::Json;
use serde_json::{
    json,
    Value,
};
use sk_core::jsonutils;
use sk_core::k8s::{
    KubeResourceExt,
    PodExt,
    PodLifecycleData,
};
use sk_core::prelude::*;

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
#[instrument(parent=None, skip_all)]
pub async fn handler(
    ctx: &rocket::State<DriverContext>,
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
        resp = mutate_pod(ctx, resp, pod, mut_data).await.unwrap_or_else(|err| {
            error!("could not perform mutation, blocking pod object: {err:?}");
            AdmissionResponse::from(&req).deny(err)
        });
    }

    Json(into_pod_review(resp))
}

// TODO when we get the pod object, the final name hasn't been filled in yet; make sure this
// doesn't cause any problems
#[instrument(skip_all, fields(pod.namespaced_name=pod.namespaced_name()))]
pub async fn mutate_pod(
    ctx: &DriverContext,
    resp: AdmissionResponse,
    pod: &corev1::Pod,
    mut_data: &MutationData,
) -> anyhow::Result<AdmissionResponse> {
    // enclose in a block so we release the mutex when we're done
    let owners = {
        let mut owners_cache = ctx.owners_cache.lock().await;
        owners_cache.compute_owner_chain(pod).await?
    };

    if !owners.iter().any(|o| o.name == ctx.root_name) {
        info!("pod not owned by simulation, no mutation performed");
        return Ok(resp);
    }

    let mut patches = vec![];
    add_simulation_labels(ctx, pod, &mut patches)?;
    add_lifecycle_annotation(ctx, pod, &owners, mut_data, &mut patches)?;
    add_node_selector_tolerations(pod, &mut patches)?;

    Ok(resp.with_patch(Patch(patches))?)
}

fn add_simulation_labels(ctx: &DriverContext, pod: &corev1::Pod, patches: &mut Vec<PatchOperation>) -> EmptyResult {
    if pod.metadata.labels.is_none() {
        patches.push(PatchOperation::Add(AddOperation { path: "/metadata/labels".into(), value: json!({}) }));
    }
    patches.push(PatchOperation::Add(AddOperation {
        path: format!("/metadata/labels/{}", escape(SIMULATION_LABEL_KEY)),
        value: Value::String(ctx.name.clone()),
    }));

    Ok(())
}

fn add_lifecycle_annotation(
    ctx: &DriverContext,
    pod: &corev1::Pod,
    owners: &Vec<metav1::OwnerReference>,
    mut_data: &MutationData,
    patches: &mut Vec<PatchOperation>,
) -> EmptyResult {
    if let Some(orig_ns) = pod.annotations().get(ORIG_NAMESPACE_ANNOTATION_KEY) {
        for owner in owners {
            let owner_ns_name = format!("{}/{}", orig_ns, owner.name);
            if !ctx.store.has_obj(&owner_ns_name) {
                continue;
            }

            let hash = jsonutils::hash(&serde_json::to_value(&pod.stable_spec()?)?);
            let seq = mut_data.count(hash);

            let lifecycle = ctx.store.lookup_pod_lifecycle(&owner_ns_name, hash, seq);
            if let Some(patch) = to_annotation_patch(&lifecycle) {
                info!("applying lifecycle annotations (hash={hash}, seq={seq})");
                if pod.metadata.annotations.is_none() {
                    patches.push(PatchOperation::Add(AddOperation {
                        path: "/metadata/annotations".into(),
                        value: json!({}),
                    }));
                }
                patches.push(patch);
                break;
            } else {
                warn!("no pod lifecycle data found");
            }
        }
    }

    Ok(())
}

fn add_node_selector_tolerations(pod: &corev1::Pod, patches: &mut Vec<PatchOperation>) -> EmptyResult {
    if pod.spec()?.tolerations.is_none() {
        patches.push(PatchOperation::Add(AddOperation { path: "/spec/tolerations".into(), value: json!([]) }));
    }
    patches.push(PatchOperation::Add(AddOperation {
        path: "/spec/nodeSelector".into(),
        value: json!({"type": "virtual"}),
    }));
    patches.push(PatchOperation::Add(AddOperation {
        path: "/spec/tolerations/-".into(),
        value: json!({"key": VIRTUAL_NODE_TOLERATION_KEY, "operator": "Exists", "effect": "NoSchedule"}),
    }));

    Ok(())
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

fn to_annotation_patch(pld: &PodLifecycleData) -> Option<PatchOperation> {
    match pld {
        PodLifecycleData::Empty | PodLifecycleData::Running(_) => None,
        PodLifecycleData::Finished(start_ts, end_ts) => Some(PatchOperation::Add(AddOperation {
            path: format!("/metadata/annotations/{}", escape(LIFETIME_ANNOTATION_KEY)),
            value: Value::String(format!("{}", end_ts - start_ts)),
        })),
    }
}
