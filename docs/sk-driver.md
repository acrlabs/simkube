---
project: SimKube
template: docs.html
---

# SimKube Simulation Driver

The SimKube driver is a job that is launched whenever a new simulation is started.  It reads all the contents of a
cluster trace file, and then replays those events on a simulated cluster.  It consists of two components, a "runner"
which replays the trace and "mutator" which intercepts simulated pods and applies the appropriate labels, node
selectors, and tolerations to ensure that the simulated pods end up on virtual nodes.

## Usage

```
Usage: sk-driver [OPTIONS] --sim-name <SIM_NAME> --sim-root <SIM_ROOT> --virtual-ns-prefix <VIRTUAL_NS_PREFIX> \
       --cert-path <CERT_PATH> --key-path <KEY_PATH> --trace-path <TRACE_PATH>

Options:
      --sim-name <SIM_NAME>
      --sim-root <SIM_ROOT>
      --virtual-ns-prefix <VIRTUAL_NS_PREFIX>
      --admission-webhook-port <ADMISSION_WEBHOOK_PORT>  [default: 8888]
      --cert-path <CERT_PATH>
      --key-path <KEY_PATH>
      --trace-path <TRACE_PATH>
  -v, --verbosity <VERBOSITY>                            [default: info]
  -h, --help                                             Print help
```

## Details

The driver is launched by the [Simulation Controller](./sk-ctrl.md) when a new simulation is started.  On startup, it
reads the cluster trace from the specified `--trace-path` and then replays all the events in the trace.  The driver
shuts down when the trace is finished.

The driver also exposes a `/mutate` endpoint on the specified `--admission-webhook-port`, which is called by the
Kubernetes control plane whenever a new pod is created.  The mutation endpoint checks to see if the Pod is owned by any
of the simulated resources, and if so, adds the following mutations to the object to ensure that it is scheduled on the
virtual cluster:

```yaml
labels:
  simkube.io/simulation: <simulation-name>
annotations:
  simkube.io/lifetime-seconds: <pod-lifetime> (if present in the trace)
spec:
  tolerations:
    - key: simkube.io/virtual-node
      value: true
  nodeSelector:
    type: virtual
```

When the simulation is over, the driver deletes the specified SimulationRoot custom resource, which cleans up all of the
simulation objects in the cluster.
