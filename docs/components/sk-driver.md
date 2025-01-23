<!--
template: docs.html
-->

# SimKube Simulation Driver

The SimKube driver is a job that is launched whenever a new simulation is started.  It reads all the contents of a
cluster trace file, and then replays those events on a simulated cluster.  It consists of two components, a "runner"
which replays the trace and "mutator" which intercepts simulated pods and applies the appropriate labels, node
selectors, and tolerations to ensure that the simulated pods end up on virtual nodes.

## Usage

```bash exec="on" result="plain"
sk-driver --help
```

## Details

The driver is launched by the [Simulation Controller](./sk-ctrl.md) when a new simulation is started.  The driver
performs the following steps:

0. Runs all preRun hooks
1. Creates the mutating webhook listener endpoint
2. Creates a SimulationRoot object to hang all simulation objects off of
3. Reads the specified trace from the specified path
4. Replays the trace events
5. Cleans up the SimulationRoot
6. Shuts down the mutating webhook listener
7. Runs all postRun hooks

The driver exposes a `/mutate` endpoint on the specified `--admission-webhook-port`, which is called by the Kubernetes
control plane whenever a new pod is created.  The mutation endpoint checks to see if the Pod is owned by any of the
simulated resources, and if so, adds the following mutations to the object to ensure that it is scheduled on the virtual
cluster:

```yaml
labels:
  simkube.io/simulation: <simulation-name>
annotations:
  simkube.io/lifetime-seconds: <pod-lifetime> (if present in the trace)
spec:
  tolerations:
    - key: kwok-provider
      operator: Exists
      effect: NoSchedule
  nodeSelector:
    type: virtual
```

When the simulation is over, the driver deletes the specified SimulationRoot custom resource, which cleans up all of the
simulation objects in the cluster.
