use std::collections::BTreeMap;

use kube::api::TypeMeta;
use kube::discovery::ApiResource;
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use sk_core::k8s::build_object_meta;
use sk_core::macros::*;
use sk_core::prelude::*;
use tracing::*;

use crate::context::SimulationContext;


// Adapted from the "full" cert-manager CRD output from kopium

pub const DRIVER_CERT_NAME: &str = "sk-driver-cert";
pub const CERT_MANAGER_GROUP: &str = "cert-manager.io";
pub const CERT_MANAGER_VERSION: &str = "v1";
pub const CERTIFICATE_KIND: &str = "Certificate";
pub const CERTIFICATE_PLURAL: &str = "certificates";

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CertificateIssuerRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CertificateSecretTemplate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialCertificateSpec {
    pub secret_name: String,
    pub secret_template: Option<CertificateSecretTemplate>,
    pub issuer_ref: CertificateIssuerRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns_names: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
pub struct PartialCertificateStatus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_private_key_secret_name: Option<String>,
}

pub type PartialCertificate = kube::api::Object<PartialCertificateSpec, PartialCertificateStatus>;

fn api_version() -> String {
    format!("{CERT_MANAGER_GROUP}/{CERT_MANAGER_VERSION}")
}

fn api_resource() -> ApiResource {
    ApiResource {
        group: CERT_MANAGER_GROUP.into(),
        version: CERT_MANAGER_VERSION.into(),
        api_version: api_version(),
        kind: CERTIFICATE_KIND.into(),
        plural: CERTIFICATE_PLURAL.into(),
    }
}

pub async fn create_certificate_if_not_present(
    ctx: &SimulationContext,
    sim: &Simulation,
    metaroot: &SimulationRoot,
) -> EmptyResult {
    let cert_api = kube::Api::<PartialCertificate>::namespaced_with(
        ctx.client.clone(),
        &sim.spec.driver.namespace,
        &api_resource(),
    );

    let owner = metaroot;
    if cert_api.get_opt(DRIVER_CERT_NAME).await?.is_none() {
        info!(
            "creating cert-manager certificate {} using issuer {}",
            DRIVER_CERT_NAME, ctx.opts.cert_manager_issuer,
        );
        let obj = PartialCertificate {
            metadata: build_object_meta(&sim.spec.driver.namespace, DRIVER_CERT_NAME, &ctx.name, owner),
            spec: PartialCertificateSpec {
                secret_name: DRIVER_CERT_NAME.into(),
                secret_template: Some(CertificateSecretTemplate {
                    annotations: None,
                    labels: klabel!(SIMULATION_LABEL_KEY => ctx.name),
                }),
                issuer_ref: CertificateIssuerRef {
                    name: ctx.opts.cert_manager_issuer.clone(),
                    kind: Some("ClusterIssuer".into()),
                    ..Default::default()
                },
                dns_names: Some(vec![format!("{}.{}.svc", ctx.driver_svc, sim.spec.driver.namespace)]),
            },
            status: None,
            types: Some(TypeMeta {
                api_version: api_version(),
                kind: CERTIFICATE_KIND.into(),
            }),
        };
        cert_api.create(&Default::default(), &obj).await?;
    }

    Ok(())
}
