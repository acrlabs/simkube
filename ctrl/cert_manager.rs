use std::collections::BTreeMap;

use kube::api::TypeMeta;
use kube::discovery::ApiResource;
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use simkube::k8s::build_object_meta;
use simkube::macros::*;
use simkube::prelude::*;
use tracing::*;

pub(super) const DRIVER_CERT_NAME: &str = "sk-driver-cert";
const CERT_MANAGER_GROUP: &str = "cert-manager.io";
const CERT_MANAGER_VERSION: &str = "v1";
const CERTIFICATE_KIND: &str = "Certificate";
const CERTIFICATE_PLURAL: &str = "certificates";

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CertificateIssuerRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CertificateSecretTemplate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PartialCertificateSpec {
    pub secret_name: String,
    pub secret_template: Option<CertificateSecretTemplate>,
    pub issuer_ref: CertificateIssuerRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns_names: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
pub(super) struct PartialCertificateStatus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_private_key_secret_name: Option<String>,
}

pub(super) type PartialCertificate = kube::api::Object<PartialCertificateSpec, PartialCertificateStatus>;

fn api_version() -> String {
    format!("{}/{}", CERT_MANAGER_GROUP, CERT_MANAGER_VERSION)
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

pub(super) async fn create_certificate_if_not_present(
    k8s_client: kube::Client,
    namespace: &str,
    driver_svc_name: &str,
    issuer_name: &str,
    sim_name: &str,
    owner: &SimulationRoot,
) -> EmptyResult {
    let cert_api = kube::Api::<PartialCertificate>::namespaced_with(k8s_client, namespace, &api_resource());

    if cert_api.get_opt(DRIVER_CERT_NAME).await?.is_none() {
        info!(
            "creating cert-manager certificate {} for {}, using issuer {}",
            DRIVER_CERT_NAME, sim_name, issuer_name
        );
        let obj = PartialCertificate {
            metadata: build_object_meta(namespace, DRIVER_CERT_NAME, sim_name, owner)?,
            spec: PartialCertificateSpec {
                secret_name: DRIVER_CERT_NAME.into(),
                secret_template: Some(CertificateSecretTemplate {
                    annotations: None,
                    labels: klabel!(SIMULATION_LABEL_KEY = sim_name),
                }),
                issuer_ref: CertificateIssuerRef {
                    name: issuer_name.into(),
                    kind: Some("ClusterIssuer".into()),
                    ..Default::default()
                },
                dns_names: Some(vec![format!("{}.{}.svc", driver_svc_name, namespace)]),
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
