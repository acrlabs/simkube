<!--
template: docs.html
-->

# SimKube Simulation Controller

The Simulation Controller watches for new Simulation Custom Resources to be posted to the API server and then configures
a simulation to be run based on the parameters specified in the CR.  The controller itself does not actually run the
simulation, it just does setup and cleanup, and then launches an [`sk-driver`](./sk-driver.md) Kubernetes Job to
actually perform the Simulation.

## Usage

```bash exec="on" result="plain"
sk-ctrl --help
```

## Details

The Simulation Controller does the following on receipt of a new Simulation:

0. Runs all preStart hooks
1. Verifies that all the expected pre-existing objects are present in the cluster
2. Creates a SimulationRoot "meta" object to hang objects off of that should persist for the whole simulation
3. Creates the namespace for the simulation driver to run in
4. Creates custom resources for the [Prometheus operator](https://prometheus-operator.dev) to configure metrics
   collection
5. Creates a MutatingWebhookConfiguration for the simulation driver
6. Creates a Service for the simulation driver
7. Sets up certificates for the simulation driver mutating webhook (currently requires the use of
   [cert-manager](https://cert-manager.io)).
8. Creates the simulation driver Job
9. Waits for the driver to complete
10. Cleans up all "meta" resources
11. Runs all postStop hooks

## Simulation Custom Resource

Simulations are controlled by a Simulation custom resource object, which specifies, among other things, how to configure
the Simulation driver, metrics collection, and any hooks.  The Simulation CR is cluster-namespaced, because it must
create SimulationRoots.

## SimulationRoot Custom Resource

The SimulationRoot CR is an empty object that is used to hang all the simulated objects off of for easy cleanup (instead
of having to write our own cleanup code, we just delete the SimulationRoot object and allow the Kubernetes garbage
collector to clean up everything it owns).  The SimulationRoot is cluster-namespaced because during the course of
simulation we may be creating additional namespaces to run simulated pods in.  Note that the driver itself _is not_
owned by the SimulationRoot, so that users can still see the results and logs from the after the sim is over.

## Configuring Metrics Collection

> [!NOTE] In the future we may move metrics collection out of SimKube proper and instead run it as a standard "hook".
> If you do not want to use Prometheus for metrics collection, or wish to configure it differently, you can disable
> metrics collection using `skctl --disable-metrics` and configure your own metrics solution with a preStart hook.

SimKube depends on the [Prometheus operator](https://prometheus-operator.dev) being installed in your simulation
cluster, as it creates custom resources understood by this operator.  The `metricsConfig` section of the Simulation spec
controls how this is set up.  The `namespace` and the `serviceAccount` fields are the namespace and service account that
the Prometheus operator uses.

SimKube will spawn a new Prometheus pod with extremely high resolution (currently 1 second) for the duration of the
simulation.  The Prometheus pod will be torn down at the end of the simulation, so it is recommended that you configure
at least one [remote write](https://prometheus.io/docs/prometheus/latest/configuration/configuration/#remote_write)
target for Prometheus for long-term metrics storage.  The `remoteWriteConfigs` section is simply a list of
[`RemoteWriteSpec`](https://prometheus-operator.dev/docs/operator/api/#monitoring.coreos.com/v1.RemoteWriteSpec) objects
from the Prometheus operator API.  You can configure as many of these as you want using any supported Prometheus remote
write target.

One suggested approach is to use [prom2parquet](https://github.com/acrlabs/prom2parquet) in order to save the Prometheus
timeseries data to S3 in the [Parquet](https://parquet.apache.org) columnar data format.
