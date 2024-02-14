use crate::k8s::split_namespaced_name;
use crate::prelude::*;

async fn get_query_strings(client: kube::Client, cm_ns_name: &str) -> anyhow::Result<Vec<String>> {
    let (cm_ns, cm_name) = split_namespaced_name(cm_ns_name);
    let cm_api: kube::Api<corev1::ConfigMap> = kube::Api::namespaced(client, &cm_ns);
    let cm = cm_api.get(&cm_name).await?;

    Ok(cm
        .data
        .unwrap_or_default()
        .get(METRIC_CONFIG_MAP_QUERY_KEY)
        .unwrap_or(&String::new())
        .lines()
        .map(|line| line.into())
        .collect())
}

pub async fn export_metrics(k8s_client: kube::Client, sim: &Simulation) -> EmptyResult {
    let prom_client = prometheus_http_query::Client::default();

    for q in get_query_strings(k8s_client, &sim.spec.metric_query_configmap).await? {
        let resp = prom_client.query(q).get().await?;
        println!("{:?}", resp.data().as_vector());
    }
    Ok(())
}
