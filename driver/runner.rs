use std::cmp::max;
use std::time::Duration;

use anyhow::anyhow;
use kube::api::{
    DynamicObject,
    Patch,
    PatchParams,
};
use kube::ResourceExt;
use serde_json::json;
use simkube::api::v1::build_simulation_root;
use simkube::jsonutils;
use simkube::k8s::{
    add_common_metadata,
    build_global_object_meta,
    ApiSet,
    GVK,
};
use simkube::macros::*;
use simkube::prelude::*;
use tokio::time::sleep;
use tracing::*;

use super::*;

pub(super) fn build_virtual_ns(
    ctx: &DriverContext,
    root: &SimulationRoot,
    namespace: &str,
) -> anyhow::Result<corev1::Namespace> {
    let owner = root;
    let mut ns = corev1::Namespace {
        metadata: build_global_object_meta(namespace, &ctx.name, owner)?,
        ..Default::default()
    };
    klabel_insert!(ns, VIRTUAL_LABEL_KEY => "true");

    Ok(ns)
}

pub(super) fn build_virtual_obj(
    ctx: &DriverContext,
    root: &SimulationRoot,
    original_ns: &str,
    virtual_ns: &str,
    obj: &DynamicObject,
    pod_spec_template_path: &str,
) -> anyhow::Result<DynamicObject> {
    let owner = root;
    let mut vobj = obj.clone();
    add_common_metadata(&ctx.name, owner, &mut vobj.metadata)?;
    vobj.metadata.namespace = Some(virtual_ns.into());
    klabel_insert!(vobj, VIRTUAL_LABEL_KEY => "true");

    jsonutils::patch_ext::add(pod_spec_template_path, "metadata", &json!({}), &mut vobj.data, false)?;
    jsonutils::patch_ext::add(
        &format!("{}/metadata", pod_spec_template_path),
        "annotations",
        &json!({}),
        &mut vobj.data,
        false,
    )?;
    jsonutils::patch_ext::add(
        &format!("{}/metadata/annotations", pod_spec_template_path),
        ORIG_NAMESPACE_ANNOTATION_KEY,
        &json!(original_ns),
        &mut vobj.data,
        true,
    )?;
    jsonutils::patch_ext::remove("", "status", &mut vobj.data)?;

    // We remove all container ports from the pod specification just before applying, because it is
    // _possible_ to create a pod with duplicate container ports, but the apiserver will _reject_ a
    // patch containing duplicate container ports.  Since pods are mocked out _anyways_ there's no
    // reason to expose the ports.  We do this here because we still want the ports to be a part of
    // the podspec when we're computing its hash, i.e., changes to the container ports will still
    // result in changes to the pod in the trace/simulation
    jsonutils::patch_ext::remove(&format!("{}/spec/containers/*", pod_spec_template_path), "ports", &mut vobj.data)?;

    Ok(vobj)
}

pub struct TraceRunner {
    roots_api: kube::Api<SimulationRoot>,
    ns_api: kube::Api<corev1::Namespace>,
    apiset: ApiSet,
}

impl TraceRunner {
    pub async fn new(client: kube::Client) -> anyhow::Result<TraceRunner> {
        Ok(TraceRunner {
            roots_api: kube::Api::all(client.clone()),
            ns_api: kube::Api::all(client.clone()),
            apiset: ApiSet::new(client.clone()),
        })
    }

    #[instrument(parent=None, skip_all, fields(simulation=ctx.name))]
    pub async fn run(mut self, ctx: DriverContext) -> EmptyResult {
        let root_obj = if let Some(root) = self.roots_api.get_opt(&ctx.root_name).await? {
            warn!("Driver root {} already exists; continuing...", ctx.root_name);
            root
        } else {
            let root_obj = build_simulation_root(&ctx.root_name, &ctx.sim)?;
            self.roots_api.create(&Default::default(), &root_obj).await?
        };

        let mut sim_ts = ctx.store.start_ts().ok_or(anyhow!("no trace data"))?;
        for (evt, maybe_next_ts) in ctx.store.iter() {
            // We're currently assuming that all tracked objects are namespace-scoped,
            // this will panic/fail if that is not true.
            for obj in &evt.applied_objs {
                let gvk = GVK::from_dynamic_obj(obj)?;
                let original_ns = obj.namespace().unwrap();
                let virtual_ns = format!("{}-{}", ctx.virtual_ns_prefix, original_ns);

                if self.ns_api.get_opt(&virtual_ns).await?.is_none() {
                    info!("creating virtual namespace: {virtual_ns}");
                    let vns = build_virtual_ns(&ctx, &root_obj, &virtual_ns)?;
                    self.ns_api.create(&Default::default(), &vns).await?;
                }

                let pod_spec_template_path = ctx
                    .store
                    .config()
                    .pod_spec_template_path(&gvk)
                    .ok_or(anyhow!("unknown simulated object: {:?}", gvk))?;
                let vobj = build_virtual_obj(&ctx, &root_obj, &original_ns, &virtual_ns, obj, pod_spec_template_path)?;

                info!("applying object {}", vobj.namespaced_name());
                self.apiset
                    .namespaced_api_for(&gvk, virtual_ns)
                    .await?
                    .patch(&vobj.name_any(), &PatchParams::apply("simkube"), &Patch::Apply(&vobj))
                    .await?;
            }

            for obj in &evt.deleted_objs {
                info!("deleting object {}", obj.namespaced_name());
                let gvk = GVK::from_dynamic_obj(obj)?;
                let virtual_ns = format!("{}-{}", ctx.virtual_ns_prefix, obj.namespace().unwrap());
                self.apiset
                    .namespaced_api_for(&gvk, virtual_ns)
                    .await?
                    .delete(&obj.name_any(), &Default::default())
                    .await?;
            }

            if let Some(next_ts) = maybe_next_ts {
                let sleep_duration = max(0, next_ts - sim_ts);

                info!("next event happens in {sleep_duration} seconds, sleeping");
                debug!("current sim ts = {sim_ts}, next sim ts = {next_ts}");

                sim_ts = next_ts;
                sleep(Duration::from_secs(sleep_duration as u64)).await;
            }
        }

        if let Err(e) = self.roots_api.delete(&ctx.root_name, &Default::default()).await {
            error!("could not delete driver root {}: {e}", ctx.root_name);
        }

        Ok(())
    }
}
