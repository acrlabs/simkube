<!--
project: SimKube
template: docs.html
-->

# skctl

`skctl` is the CLI for interacting with SimKube.  It's not required to use but it will make your life a lot easier.

```
command-line app for running simulations with SimKube

Usage: skctl <COMMAND>

Commands:
  crd       print SimKube CRDs
  delete    delete a simulation
  export    export simulation trace data
  run       run a simulation
  snapshot  take a point-in-time snapshot of a cluster (does not require sk-tracer to be running)
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## skctl crd

```
print SimKube CRDs

Usage: skctl crd

Options:
  -h, --help     Print help
  -V, --version  Print version
```

Generate all of the necessary CustomResourceDefinitions for SimKube.

## skctl delete

```
delete a simulation

Usage: skctl delete --name <NAME>

Options:
  -n, --name <NAME>
          name of the simulation to delete

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## skctl export

```
export simulation trace data

Usage: skctl export [OPTIONS]

Options:
      --start-time <START_TIME>
          trace export start timestamp; can be a relative duration
          or absolute timestamp; durations are computed relative
          to the specified end time, _not_ the current time

          [default: -30m]

      --end-time <END_TIME>
          end time; can be a relative or absolute timestamp

          [default: now]

      --excluded-namespaces <EXCLUDED_NAMESPACES>
          namespaces to exclude from the trace

          [default: cert-manager,kube-system,local-path-storage,monitoring,simkube]

      --tracer-address <TRACER_ADDRESS>
          sk-tracer server address

          [default: http://localhost:7777]

      --output <OUTPUT>
          location to save exported trace

          [default: file:///tmp/kind-node-data]

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

```

Export a trace from a running `sk-tracer` pod between the specified `--start-time` and `--end-time`, as well as
according to the specified filters.  The resulting trace will be stored in the `--output` directory.  Timestamps
can either be relative ("-2h", "now", etc) or absolute ("2024-01-01T12:00:00").  If you find a timestamp format
that isn't accepted or is parsed incorrectly, please [file an issue](https://github.com/acrlabs/simkube/issues/new?template=bug_report.md&title=incorrect%20timestamp%20parsing&labels=cli,bug).

## skctl run

```
run a simulation

Usage: skctl run [OPTIONS] --name <NAME> [DURATION]

Arguments:
  [DURATION]
          duration of the simulation

Options:
  -n, --name <NAME>
          name of the simulation to run

      --trace-file <TRACE_FILE>
          location of the trace file for sk-driver to read

          [default: file:///data/trace]

      --driver-namespace <DRIVER_NAMESPACE>
          namespace to launch sk-driver in

          [default: simkube]

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

Help:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## skctl snapshot

```
take a point-in-time snapshot of a cluster (does not require sk-tracer to be running)

Usage: skctl snapshot [OPTIONS] --config-file <CONFIG_FILE>

Options:
  -c, --config-file <CONFIG_FILE>
          config file specifying resources to snapshot

      --excluded-namespaces <EXCLUDED_NAMESPACES>
          namespaces to exclude from the snapshot

          [default: cert-manager,kube-system,local-path-storage,monitoring,simkube]

      --output <OUTPUT>
          location to save exported trace

          [default: trace.out]

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

Create a point-in-time snapshot of the configured objects that are currently running on a Kubernetes cluster.  Note
that, unlike `skctl export`, the snapshot command _does not require `sk-tracer` to be running on the cluster!_  This
means that you can pick an arbitrary starting point, create a trace file from there, and "let the simulation run to see
what happens".  The snapshot command will try to read your local Kubernetes credentials from, e.g., `~/.kube/config`,
and you will need read access to all namespaces on the cluster you're trying to snapshot.

The config file format is the same as for [sk-tracer](sk-tracer.md); there is an example in the [examples
folder](https://github.com/acrlabs/simkube/blob/master/examples/tracer_config.yml).
