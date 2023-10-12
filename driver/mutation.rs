use json_patch::{
    AddOperation,
    Patch,
    PatchOperation,
};
use k8s_openapi::api::core::v1 as corev1;
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
use simkube::jsonutils;
use simkube::prelude::*;
use tracing::*;

use super::*;

#[rocket::post("/", data = "<body>")]
pub async fn handler(
    ctx: &rocket::State<DriverContext>,
    body: Json<AdmissionReview<corev1::Pod>>,
) -> Json<AdmissionReview<corev1::Pod>> {
    let req: AdmissionRequest<_> = match body.into_inner().try_into() {
        Ok(r) => r,
        Err(e) => {
            error!("could not parse request: {}", e);
            let resp = AdmissionResponse::invalid(e);
            return Json(into_pod_review(resp));
        },
    };

    let mut resp = AdmissionResponse::from(&req);
    if let Some(pod) = &req.object {
        info!("received mutation request for pod: {}", pod.namespaced_name());
        resp = match mutate_pod(ctx, resp, pod).await {
            Ok(r) => {
                info!("mutation successfully constructed");
                r
            },
            Err(e) => {
                error!("could not perform mutation, blocking pod object: {}", e);
                AdmissionResponse::from(&req).deny(e)
            },
        };
    }

    Json(into_pod_review(resp))
}

// TODO when we get the pod object, the final name hasn't been filled in yet; make sure this
// doesn't cause any problems
pub(super) async fn mutate_pod(
    ctx: &rocket::State<DriverContext>,
    resp: AdmissionResponse,
    pod: &corev1::Pod,
) -> anyhow::Result<AdmissionResponse> {
    {
        // enclose in a block so we release the mutex when we're done
        let mut owners_cache = ctx.owners_cache.lock().await;
        let owners = owners_cache.compute_owner_chain(pod).await?;

        if owners.iter().all(|o| o.name != ctx.sim_root_name) {
            return Ok(resp);
        }
    }

    let mut patches = vec![];
    if pod.metadata.labels.is_none() {
        patches.push(PatchOperation::Add(AddOperation { path: "/metadata/labels".into(), value: json!({}) }));
    }

    if pod.spec()?.tolerations.is_none() {
        patches.push(PatchOperation::Add(AddOperation { path: "/spec/tolerations".into(), value: json!([]) }));
    }

    patches.extend(vec![
        PatchOperation::Add(AddOperation {
            path: format!("/metadata/labels/{}", jsonutils::escape(SIMULATION_LABEL_KEY)),
            value: Value::String(ctx.sim_name.clone()),
        }),
        PatchOperation::Add(AddOperation {
            path: "/spec/nodeSelector".into(),
            value: json!({"type": "virtual"}),
        }),
        PatchOperation::Add(AddOperation {
            path: "/spec/tolerations/-".into(),
            value: json!({"key": VIRTUAL_NODE_TOLERATION_KEY, "value": "true"}),
        }),
    ]);
    Ok(resp.with_patch(Patch(patches))?)
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
