<!--
template: docs.html
-->
# Evaluate your results
Prometheus and Grafana are installed natively. Users can view simulation results by connecting to the Grafana pod on your EC2 instance:

## 1 Set up port forwarding:

```sh
kubectl port-forward -n monitoring svc/grafana 3000
```

## 2 Open the Grafana UI
<http://localhost:3000/>

## 3 Create a Dashboard

- `Dashboards > New > New Dashboard > Add visualization`
- In the `Data source` field select `prometheus`
- In the `Query` field select `Code`
- Enter your PromQL query

Here are some queries to try:

#### See all simulated pods over time
```promql
sum(kube_pod_status_phase{phase="Running", namespace=~"virtual-.*"})
```

#### See all virtual KWOK nodes by instance type
```promql
sum(kube_node_status_condition{condition="Ready", status="true"} * on (node) group_right kube_pod_labels{label_type="virtual"}) by (label_node_kubernetes_io_instance_type)
```
