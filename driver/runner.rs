use std::cmp::max;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use k8s_openapi::api::core::v1 as corev1;
use kube::api::{
    DynamicObject,
    Patch,
    PatchParams,
};
use kube::ResourceExt;
use simkube::jsonutils;
use simkube::k8s::{
    add_common_metadata,
    build_global_object_meta,
    prefixed_ns,
    ApiSet,
    GVK,
};
use simkube::macros::*;
use simkube::prelude::*;
use simkube::store::TraceStore;
use tokio::runtime::Handle;
use tokio::task::block_in_place;
use tokio::time::sleep;
use tracing::*;

fn build_virtual_ns(sim_name: &str, ns_name: &str, sim_root: &SimulationRoot) -> anyhow::Result<corev1::Namespace> {
    let mut ns = corev1::Namespace {
        metadata: build_global_object_meta(ns_name, sim_name, sim_root)?,
        ..Default::default()
    };
    klabel_insert!(ns, VIRTUAL_LABEL_KEY = "true");

    Ok(ns)
}

fn build_virtual_obj(
    obj: &DynamicObject,
    vns_name: &str,
    sim_name: &str,
    root: &SimulationRoot,
) -> anyhow::Result<DynamicObject> {
    let mut vobj = obj.clone();

    vobj.metadata.namespace = Some(vns_name.into());
    jsonutils::patch_ext::remove("", "status", &mut vobj.data)?;
    klabel_insert!(vobj, VIRTUAL_LABEL_KEY = "true");

    add_common_metadata(sim_name, root, &mut vobj.metadata)?;

    Ok(vobj)
}

pub struct TraceRunner {
    store: Arc<TraceStore>,
    sim_root: SimulationRoot,
    ns_prefix: String,
    sim_name: String,
    client: kube::Client,
}

impl TraceRunner {
    pub async fn new(
        sim_name: &str,
        sim_root: &str,
        store: Arc<TraceStore>,
        ns_prefix: &str,
    ) -> anyhow::Result<TraceRunner> {
        let client = kube::Client::try_default().await?;
        let roots_api: kube::Api<SimulationRoot> = kube::Api::all(client.clone());
        let root = roots_api.get(sim_root).await?;

        Ok(TraceRunner {
            sim_name: sim_name.into(),
            sim_root: root,
            store,
            ns_prefix: ns_prefix.into(),
            client,
        })
    }

    pub async fn run(self) -> EmptyResult {
        info!("starting simulation {}", self.sim_name);
        let ns_api: kube::Api<corev1::Namespace> = kube::Api::all(self.client.clone());
        let mut apiset = ApiSet::new(self.client.clone());
        let mut sim_ts = self.store.start_ts().ok_or(anyhow!("no trace data"))?;
        for (evt, next_ts) in self.store.iter() {
            for obj in evt.applied_objs {
                let gvk = GVK::from_dynamic_obj(&obj)?;
                let vns_name = prefixed_ns(&self.ns_prefix, &obj);
                let vobj = build_virtual_obj(&obj, &vns_name, &self.sim_name, &self.sim_root)?;

                if ns_api.get_opt(&vns_name).await?.is_none() {
                    info!("creating virtual namespace: {}", vns_name);
                    let vns = build_virtual_ns(&self.sim_name, &vns_name, &self.sim_root)?;
                    ns_api.create(&Default::default(), &vns).await?;
                }

                info!("applying object {}", vobj.namespaced_name());
                apiset
                    .namespaced_api_for(&gvk, vns_name)
                    .await?
                    .patch(&vobj.name_any(), &PatchParams::apply("simkube"), &Patch::Apply(&vobj))
                    .await?;
            }

            for obj in evt.deleted_objs {
                info!("deleting object {}", obj.namespaced_name());
                let gvk = GVK::from_dynamic_obj(&obj)?;
                let vns_name = prefixed_ns(&self.ns_prefix, &obj);
                apiset
                    .namespaced_api_for(&gvk, vns_name)
                    .await?
                    .delete(&obj.name_any(), &Default::default())
                    .await?;
            }

            if let Some(ts) = next_ts {
                let sleep_duration = max(0, ts - sim_ts);
                sim_ts = ts;
                info!("next event happens in {} seconds, sleeping", sleep_duration);
                sleep(Duration::from_secs(sleep_duration as u64)).await;
            }
        }

        info!("simulation complete!");

        Ok(())
    }
}

impl Drop for TraceRunner {
    fn drop(&mut self) {
        info!("cleaning up simulation {}", self.sim_name);
        let roots_api: kube::Api<SimulationRoot> = kube::Api::all(self.client.clone());
        let _ = block_in_place(|| {
            Handle::current().block_on(roots_api.delete(&self.sim_root.name_any(), &Default::default()))
        });
    }
}
