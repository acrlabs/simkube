<!--
project: SimKube
template: docs.html
-->

# Metrics Collection and Data Analysis

Collecting metrics from a simulation is a complicated subject, and discussing the wide variety of ways users may
configure this is well outside the scope of these docs.  However, SimKube does have some helpers to try to make metrics
collection easier.  In this section we will discuss one possible way users can configure metrics collection.

## Prerequisites

If you want to use SimKube's built-in metrics collection helpers, you must install the [Promtheus operator](https://github.com/prometheus-operator/prometheus-operator);
 we recommend configuring this via the [kube-prometheus](https://github.com/prometheus-operator/kube-prometheus)
project.  To ensure that metrics are persisted beyond the time of your simulation, you will need to configure a
[remote write endpoint](https://prometheus.io/docs/prometheus/latest/configuration/configuration/#remote_write).
One option here is the [prom2parquet writer](https://github.com/acrlabs/prom2parquet).

## What metrics to scrape?

Due to the incredibly large number of metrics produced by the Kubernetes control plane and supporting components, most
monitoring tools aggressively limit the data that is ingested, either by restricting the metrics that are scraped, by
increasing the scrape interval, by pre-aggregating metric values with recording rules, or some combination of the three.
However, for simulation purposes, many times we want to have extremely high-resolution data collection so that we know
_exactly_ what occurred during the simulation.  Running such high-resolution sampling permanently is cost-prohibitive in
most cases, so SimKube instead provides tools for only collecting high-resolution data when a simulation is being run,
and then using Prometheus's built-in remote write functionality to persist the data for long-term storage.

To tell the Prometheus pod spawned by SimKube what metrics to scrape and at what frequency, you need to create "monitor"
objects in your cluster.  The Prometheus operator installs a few different kinds of monitors, including
[ServiceMonitors](https://prometheus-operator.dev/docs/operator/api/#monitoring.coreos.com/v1.ServiceMonitor), which
tell Prometheus to scrape a metrics endpoint exposed by a particular Kubernetes Service, and
[PodMonitors](https://prometheus-operator.dev/docs/operator/api/#monitoring.coreos.com/v1.PodMonitor), which do the same
but targeted at specific pods.  Within these monitoring resources, you can configure the `scrapeInterval` and `selector`
fields, to tell Prometheus exactly what pods or services to scrape and how frequently.  Additionally, you can define
your own custom [relabel configs](https://prometheus.io/docs/prometheus/latest/configuration/configuration/#relabel_config)
to customize which targets to scrape, as well as [metric relabel configs](https://prometheus.io/docs/prometheus/latest/configuration/configuration/#metric_relabel_configs)
to customize the timeseries data that is scraped.

Defining a comprehensive set of monitoring resources is beyond the scope of this documentation and we recommend that the
user carefully read the [Prometheus operator documentation](https://prometheus-operator.dev/docs/user-guides/getting-started/)
 for more details.  However, a basic recommendation is that you place your monitor resources in a separate namespace
that is not watched by your default Prometheus install, so you do not overwhelm its storage (SimKube defaults to
`monitoring-hd` for this namespace).  A set of example Pod- and ServiceMonitor configurations is given in the
[examples](https://github.com/acrlabs/simkube/tree/master/examples/metrics) of the SimKube repository.

Once you have set up your monitoring resources, you can point the Prometheus pod to them via the `metricsConfig` section
of your Simulation spec:

```yaml
  metricsConfig:
    namespace: default-monitoring-ns
    podMonitorNamespaces:
      # list of namespaces
    podMonitorNames:
      # list of names
    serviceMonitorNamespaces:
      # list of namespaces
    serviceMonitorNames:
      # list of names
    shards: 1
```

If `podMonitorNamespaces` or `serviceMonitorNamespaces` are blank, the Prometheus pod will default to discovering these
resources in the value specified under `metricsConfig.namespace` (this is also the namespace where SimKube will create
the Prometheus pod).  If `podMonitorNames` or `serviceMonitorNames` are blank, the Prometheus pod will read all monitor
resources in the specified namespaces.

Lastly, because collecting high-resolution samples from so many different sources can be quite time- and
resource-intensive, you can configure the number of Prometheus shards launched by SimKube.  Increasing the number of
shards can make metrics collection more accurate at the expense of requiring more resources.

## What to do with scraped metrics?

The Prometheus pod(s) launched by SimKube will be torn down at the end of the simulation, which means the data needs to
be persisted somewhere for long-term storage and analysis.  SimKube will configure Prometheus to save the data to any
remote write endpoints you like, via the `remoteWriteConfigs` section of the Simulation metrics config spec.  The format
for this config is given in the [Prometheus operator API](https://prometheus-operator.dev/docs/operator/api/#monitoring.coreos.com/v1.RemoteWriteSpec).

As above, configuring your remote write endpoints is beyond the scope of this document; however, one option is the
[prom2parquet](https://github.com/acrlabs/prom2parquet) writer, which will save all your endpoints to S3 in the Parquet
format for further analysis.

## Configuring metrics collection with `skctl`

If you don't want to hand-craft a Simulation resource specification, you can run a simulation with all of these options
configured using the `skctl` utility.  It allows you to specify each of these options via command-line flags, or it can
use sensible default values:

```
> skctl run --help

...

Metrics:
      --disable-metrics
          don't spawn Prometheus pod before running sim

      --metrics-namespace <METRICS_NAMESPACE>
          namespace to launch monitoring utilities in

          [default: monitoring]

      --metrics-service-account <METRICS_SERVICE_ACCOUNT>
          service account with monitoring permissions

          [default: prometheus-k8s]

      --metrics-pod-monitor-namespaces <METRICS_POD_MONITOR_NAMESPACES>
          comma-separated list of namespaces containing pod monitor configs

          [default: monitoring-hd]

      --metrics-pod-monitor-names <METRICS_POD_MONITOR_NAMES>
          comma-separated list of pod monitor config names
          (if empty, uses all pod monitor configs in metrics_pod_monitor_namespaces)

      --metrics-service-monitor-namespaces <METRICS_SERVICE_MONITOR_NAMESPACES>
          comma-separated list of namespaces containing service monitor configs

          [default: monitoring-hd]

      --metrics-service-monitor-names <METRICS_SERVICE_MONITOR_NAMES>
          comma-separated list of service monitor config names
          (if empty, uses all pod monitor configs in metrics_service_monitor_namespaces)

      --prometheus-shards <PROMETHEUS_SHARDS>
          number of prometheus shards to run

      --remote-write-endpoint <REMOTE_WRITE_ENDPOINT>
          address for remote write endpoint

...
```

You can alternately completely disable metrics collection for Simulations started by `skctl run` using the
`--disable-metrics` flag.  The remote write endpoint given on the command line will be translated into the following
`RemoteWriteSpec`:

```yaml
- url: <REMOTE_WRITE_ENDPOINT>
```

If you need to configure something more complicated, you will need to create the Simulation custom resource by hand
instead of using `skctl`.

## Troubleshooting metrics collection

Figuring out what went wrong with your metrics collection pipeline can be quite cumbersome.  Here are a few things you
can try if you're not getting the metrics that you're expecting.

### controller-manager or scheduler metrics

In the default configuration for `kubeadm` (also used by `kind`), the controller-manager and scheduler are bound to
`127.0.0.1` and have no ports exposed.  In order for a Prometheus operator to be able to scrape these via a PodMonitor
object, you need to instead have them bound to `0.0.0.0` and expose the port they listen on (exposing the port
_shouldn't_ be necessary because these pods use host networking, but the Prometheus operator will ignore them as scrape
targets if their pod spec doesn't have the container port listed).  You can configure this in kind, for example, with
the following additions to your kind config:

```yaml
controllerManager:
  extraArgs:
    bind-address: 0.0.0.0
    authorization-always-allow-paths: "/healthz,/readyz,/livez,/metrics"
scheduler:
  extraArgs:
    bind-address: 0.0.0.0
    authorization-always-allow-paths: "/healthz,/readyz,/livez,/metrics"
```

The `authorization-always-allow-paths` field allows you to scrape these metrics unauthenticated.  If you don't want to
expose your metrics endpoint unauthenticated you will need to configure certificates for your Prometheus pod.

As an alternate approach, you can create Kubernetes Service objects for controller-manager and kube-scheduler, and use a
ServiceMonitor resource to scrape these instead.  This has the slight advantage that you can use the configured service
account for the Prometheus pod for authentication, but it requires some additional setup during cluster creation time
since `kubeadm` doesn't create these for you by default.

### Some other metric isn't showing up

If you can't figure out why metrics aren't showing up in Prometheus, you can start a simulation and then set up port
forwarding to inspect the Prometheus pod:

```
> export SIM_NAME=testing
> skctl run -n ${} +1h
> kubectl port-forward -n monitoring prometheus-sk-${SIM_NAME}-prom 9090
```

Then, navigate to http://localhost:9090/targets to see what targets Prometheus is currently scraping, and what problems
they may be encountering.  You can instead look at http://localhost:9090/config to inspect the Prometheus configuration
file created by the Prometheus operator, or you can go to http://localhost:9090/service-discovery to see why services
may not be discovered by Prometheus.  There are some additional troubleshooting steps you can follow in the [Prometheus
operator docs](https://prometheus-operator.dev/docs/operator/troubleshooting/#troubleshooting-servicemonitor-changes)
