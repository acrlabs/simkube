<!--
project: SimKube
template: docs.html
-->

# SimKube Simulation Controller

The Simulation Controller watches for new Simulation Custom Resources to be posted to the API server and then configures
a simulation to be run based on the parameters specified in the CR.  The controller itself does not actually run the
simulation, it just does setup and cleanup, and then launches an [`sk-driver`](./sk-driver.md) Kubernetes Job to
actually perform the Simulation.

## Usage

```
Usage: sk-ctrl [OPTIONS] --driver-image <DRIVER_IMAGE>

Options:
      --driver-image <DRIVER_IMAGE>
      --driver-port <DRIVER_PORT>                  [default: 8888]
      --use-cert-manager
      --cert-manager-issuer <CERT_MANAGER_ISSUER>  [default: ]
  -v, --verbosity <VERBOSITY>                      [default: info]
  -h, --help                                       Print help
```

## Details

The Simulation Controller does the following on receipt of a new Simulation:

1. Creates a SimulationRoot object to hang all of the simulated objects off of
2. Creates the namespace for the simulation driver to run in
3. Creates a MutatingWebhookConfiguration for the simulation driver
4. Creates a Service for the simulation driver
5. Sets up certificates for the simulation driver mutating webhook (currently requires the use of
   [cert-manager](https://cert-manager.io)).
6. Creates the simulation driver Job

## Simulation Custom Resource

Here is an example Simulation object:

```yaml
apiVersion: simkube.io/v1
kind: Simulation
metadata:
  name: testing
spec:
  driverNamespace: simkube
  trace: file:///data/trace
```

The `SimulationSpec` contains two fields, the location of the trace file which we want to use for the simulation, and
the namespace to launch the driver into.  Currently the only trace location supported is `file:///`, i.e., the trace
file already has to be present on the driver node at the specified location.  In the future we will support downloading
from an S3 bucket or other persistent storage.

The Simulation CR is cluster-namespaced, because it must create SimulationRoots.

## SimulationRoot Custom Resource

The SimulationRoot CR is an empty object that is used to hang all the simulated objects off of for easy cleanup (instead
of having to write our own cleanup code, we just delete the SimulationRoot object and allow the Kubernetes garbage
collector to clean up everything it owns).  The SimulationRoot is cluster-namespaced because during the course of
simulation we may be creating additional namespaces to run simulated pods in.  Note that the driver itself _is not_
owned by the SimulationRoot, so that users can still see the results and logs from the after the sim is over.
