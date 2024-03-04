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
    owner: &SimulationRoot,
    namespace: &str,
) -> anyhow::Result<corev1::Namespace> {
    let mut ns = corev1::Namespace {
        metadata: build_global_object_meta(namespace, &ctx.name, owner)?,
        ..Default::default()
    };
    klabel_insert!(ns, VIRTUAL_LABEL_KEY => "true");

    Ok(ns)
}

pub(super) fn build_virtual_obj(
    ctx: &DriverContext,
    owner: &SimulationRoot,
    original_ns: &str,
    virtual_ns: &str,
    obj: &DynamicObject,
    pod_spec_template_path: &str,
) -> anyhow::Result<DynamicObject> {
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


    Ok(vobj)
}

pub struct TraceRunner {
    ctx: DriverContext,
    root: SimulationRoot,

    ns_api: kube::Api<corev1::Namespace>,
    apiset: ApiSet,
}

impl TraceRunner {
    pub async fn new(client: kube::Client, ctx: DriverContext) -> anyhow::Result<TraceRunner> {
        let roots_api: kube::Api<SimulationRoot> = kube::Api::all(client.clone());
        let root = roots_api.get(&ctx.sim_root).await?;

        Ok(TraceRunner {
            ctx,
            root,
            ns_api: kube::Api::all(client.clone()),
            apiset: ApiSet::new(client.clone()),
        })
    }

    #[instrument(parent=None, skip_all, fields(simulation=self.ctx.name))]
    pub async fn run(mut self) -> EmptyResult {
        let mut sim_ts = self.ctx.store.start_ts().ok_or(anyhow!("no trace data"))?;
        for (evt, maybe_next_ts) in self.ctx.store.iter() {
            // We're currently assuming that all tracked objects are namespace-scoped,
            // this will panic/fail if that is not true.
            for obj in &evt.applied_objs {
                let gvk = GVK::from_dynamic_obj(obj)?;
                let original_ns = obj.namespace().unwrap();
                let virtual_ns = format!("{}-{}", self.ctx.virtual_ns_prefix, original_ns);

                if self.ns_api.get_opt(&virtual_ns).await?.is_none() {
                    info!("creating virtual namespace: {virtual_ns}");
                    let vns = build_virtual_ns(&self.ctx, &self.root, &virtual_ns)?;
                    self.ns_api.create(&Default::default(), &vns).await?;
                }

                let pod_spec_template_path = self
                    .ctx
                    .store
                    .config()
                    .pod_spec_template_path(&gvk)
                    .ok_or(anyhow!("unknown simulated object: {:?}", gvk))?;
                let vobj =
                    build_virtual_obj(&self.ctx, &self.root, &original_ns, &virtual_ns, obj, pod_spec_template_path)?;

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
                let virtual_ns = format!("{}-{}", self.ctx.virtual_ns_prefix, obj.namespace().unwrap());
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

        Ok(())
    }
}
