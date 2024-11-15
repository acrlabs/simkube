use std::cmp::max;
use std::time::Duration;

use anyhow::{
    anyhow,
    bail,
};
use clockabilly::{
    Clockable,
    UtcClock,
};
use either::Either;
use json_patch_ext::{
    add_operation,
    escape,
    format_ptr,
    patch_ext,
    remove_operation,
};
use kube::api::{
    DeleteParams,
    DynamicObject,
    Patch,
    PatchParams,
    PropagationPolicy,
};
use serde_json::json;
use sk_core::errors::*;
use sk_core::k8s::{
    add_common_metadata,
    build_global_object_meta,
    build_simulation_root,
    try_update_lease,
    ApiSet,
    GVK,
};
use sk_core::macros::*;
use sk_core::prelude::*;
use tokio::time::sleep;
use tracing::*;

use super::*;

pub const DRIVER_CLEANUP_TIMEOUT_SECONDS: i64 = 300;

err_impl! {SkDriverError,
    #[error("could not delete simulation root {0}")]
    CleanupFailed(String),

    #[error("timed out deleting simulation root {0}")]
    CleanupTimeout(String),
}

pub fn build_virtual_ns(ctx: &DriverContext, root: &SimulationRoot, namespace: &str) -> corev1::Namespace {
    let owner = root;
    let mut ns = corev1::Namespace {
        metadata: build_global_object_meta(namespace, &ctx.name, owner),
        ..Default::default()
    };
    klabel_insert!(ns, VIRTUAL_LABEL_KEY => "true");

    ns
}

pub fn build_virtual_obj(
    ctx: &DriverContext,
    root: &SimulationRoot,
    original_ns: &str,
    virtual_ns: &str,
    obj: &DynamicObject,
    maybe_pod_spec_template_path: Option<&str>,
) -> anyhow::Result<DynamicObject> {
    let owner = root;
    let mut vobj = obj.clone();
    add_common_metadata(&ctx.name, owner, &mut vobj.metadata);
    vobj.metadata.namespace = Some(virtual_ns.into());
    klabel_insert!(vobj, VIRTUAL_LABEL_KEY => "true");

    if let Some(pod_spec_template_path) = maybe_pod_spec_template_path {
        patch_ext(
            &mut vobj.data,
            add_operation(
                format_ptr!("{pod_spec_template_path}/metadata/annotations/{}", escape(ORIG_NAMESPACE_ANNOTATION_KEY)),
                json!(original_ns),
            ),
        )?;
        patch_ext(&mut vobj.data, remove_operation(format_ptr!("/status")))?;

        // We remove all container ports from the pod specification just before applying, because it is
        // _possible_ to create a pod with duplicate container ports, but the apiserver will _reject_ a
        // patch containing duplicate container ports.  Since pods are mocked out _anyways_ there's no
        // reason to expose the ports.  We do this here because we still want the ports to be a part of
        // the podspec when we're computing its hash, i.e., changes to the container ports will still
        // result in changes to the pod in the trace/simulation
        patch_ext(&mut vobj.data, remove_operation(format_ptr!("{pod_spec_template_path}/spec/containers/*/ports")))?;
    }

    Ok(vobj)
}

#[instrument(parent=None, skip_all, fields(simulation=ctx.name))]
pub async fn run_trace(ctx: DriverContext, client: kube::Client) -> EmptyResult {
    let roots_api: kube::Api<SimulationRoot> = kube::Api::all(client.clone());
    let ns_api: kube::Api<corev1::Namespace> = kube::Api::all(client.clone());
    let mut apiset = ApiSet::new(client.clone());

    let root_obj = if let Some(root) = roots_api.get_opt(&ctx.root_name).await? {
        warn!("Driver root {} already exists; continuing...", ctx.root_name);
        root
    } else {
        let root_obj = build_simulation_root(&ctx.root_name, &ctx.sim);
        roots_api.create(&Default::default(), &root_obj).await?
    };

    let speed = ctx.sim.spec.driver.speed;

    let mut sim_ts = ctx.store.start_ts().ok_or(anyhow!("no trace data"))?;
    let sim_end_ts = ctx.store.end_ts().ok_or(anyhow!("no trace data"))?;
    let sim_duration = sim_end_ts - sim_ts;

    try_update_lease(client.clone(), &ctx.sim, &ctx.ctrl_ns, sim_duration).await?;

    for (evt, maybe_next_ts) in ctx.store.iter() {
        // We're currently assuming that all tracked objects are namespace-scoped,
        // this will panic/fail if that is not true.
        for obj in &evt.applied_objs {
            let gvk = GVK::from_dynamic_obj(obj)?;
            let original_ns = obj.namespace().unwrap();
            let virtual_ns = format!("{}-{}", ctx.virtual_ns_prefix, original_ns);

            if ns_api.get_opt(&virtual_ns).await?.is_none() {
                info!("creating virtual namespace: {virtual_ns}");
                let vns = build_virtual_ns(&ctx, &root_obj, &virtual_ns);
                ns_api.create(&Default::default(), &vns).await?;
            }

            let pod_spec_template_path = ctx.store.config().pod_spec_template_path(&gvk);
            let vobj = build_virtual_obj(&ctx, &root_obj, &original_ns, &virtual_ns, obj, pod_spec_template_path)?;

            info!("applying object {}", vobj.namespaced_name());
            apiset
                .api_for_obj(&vobj)
                .await?
                .patch(&vobj.name_any(), &PatchParams::apply("simkube"), &Patch::Apply(&vobj))
                .await?;
        }

        for obj in &evt.deleted_objs {
            info!("deleting object {}", obj.namespaced_name());
            let virtual_ns = format!("{}-{}", ctx.virtual_ns_prefix, obj.namespace().unwrap());
            let mut vobj = obj.clone();
            vobj.metadata.namespace = Some(virtual_ns);
            apiset
                .api_for_obj(&vobj)
                .await?
                .delete(&obj.name_any(), &Default::default())
                .await?;
        }

        if let Some(next_ts) = maybe_next_ts {
            let simulation_normal_step_duration = max(0, next_ts - sim_ts) as f64;
            let sleep_duration = (simulation_normal_step_duration / speed) as u64;

            info!("next event happens in {sleep_duration} seconds, sleeping");
            debug!("current sim ts = {sim_ts}, next sim ts = {next_ts}");

            sim_ts = next_ts;
            sleep(Duration::from_secs(sleep_duration as u64)).await;
        }
    }

    let clock = UtcClock::boxed();
    let timeout = clock.now_ts() + DRIVER_CLEANUP_TIMEOUT_SECONDS;
    cleanup_trace(&ctx, roots_api, clock, timeout).await
}

pub async fn cleanup_trace(
    ctx: &DriverContext,
    roots_api: kube::Api<SimulationRoot>,
    clock: Box<dyn Clockable + Send>,
    timeout: i64,
) -> EmptyResult {
    info!("Cleaning up simulation objects...");

    let mut cleanup_done = false;
    while clock.now_ts() < timeout {
        // delete returns an "either" object; left contains the object being deleted,
        // and right contains a status code indicating the delete is finished.
        match roots_api
            .delete(
                &ctx.root_name,
                &DeleteParams {
                    propagation_policy: Some(PropagationPolicy::Foreground),
                    ..Default::default()
                },
            )
            .await
        {
            // In the situation where we delete, wait five seconds, and then everything's cleaned
            // up, the second delete call will return not found, which is not an error in this case
            Ok(Either::Right(_)) | Err(kube::Error::Api(kube::core::ErrorResponse { code: 404, .. })) => {
                cleanup_done = true;
                break;
            },
            Err(e) => {
                error!("delete failed: {e}");
                bail!(SkDriverError::cleanup_failed(&ctx.root_name));
            },
            _ => sleep(Duration::from_secs(RETRY_DELAY_SECONDS)).await,
        }
    }
    if !cleanup_done {
        bail!(SkDriverError::cleanup_timeout(&ctx.root_name));
    }
    info!("All objects deleted!");
    Ok(())
}
