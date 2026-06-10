<!--
template: docs.html
-->

# Working with bare pods

Normally, SimKube only tracks higher-level objects in its [trace files](ref/trace-files.md), such as Deployments,
StatefulSets, or CronJobs.  However, in some circumstances, it is useful for SimKube to track raw pod objects; for
example, if these pods are created by some controller that you do not want to (or cannot) install in your simulation
environment.  In these situations, you can configure SimKube to simulate _bare pods_.

## Configuring sk-tracer for bare pods

To configure `sk-tracer` to track bare pods, use the following stanza in your [trace config](ref/tracer-config.md):

```yaml
trackedObjects:
  v1.Pod:
    skipOwned: true
    trackLifecycle: true
```

> [!NOTE]
> If your pods are owned by some other Kubernetes resource (even if you are not tracking it), do not set the `skipOwned`
> field; see the warning below.

This will configure SimKube to track the bare pods in your production environment, along with their lifecycles (i.e.,
how long those pods are in a `Running` state).  The `skipOwned` config value tells the SimKube tracer to ignore any pods
that are owned by some other Kubernetes resource; this config field is not strictly necessary, but it will limit some of
the memory usage of the tracer).

> [!WARNING]
> In the current version of SimKube, this will only work if the tracked pod objects have an empty `OwnerReference`
> field; if your pods are owned by some resource that is outside the scope of the simulation, the techniques here will
> not work.  This will be fixed in a future version of SimKube; however, in the short term, you can use
> [SKEL](ref/skel.md) to edit out the `OwnerReference` field of pods, with a command like the following:
>
> `delete(kind == "Pod", spec.metadata.ownerReferences);`

## Running simulations with bare pods

In the simulated environment, any bare pods that are interrupted for any reason (for example, because Karpenter has
consolidated a node) _will not get rescheduled_.  Normally Kubernetes relies on the owning controller to recreate
interrupted pods, but by design there is no such owning controller in this scenario.  To circumvent this, you can
configure SimKube itself to act as the "owning controller".  Run your simulation with the following flag set:

```text
skctl run my-simulation --reschedule-interrupted-bare-pods
```

This flag will instruct the SimKube driver to watch for any terminated pods that are still in the `Running` phase and
recreate them.  The recreated pods have a `-clone-N` suffix appended to them so you can easily tell how many times a pod
has been recreated.  This should improve your simulation fidelity significantly in a bare pods scenario.

> [!NOTE]
> For bare pods that have lifecycle events configured, the rescheduled pod will _start over_; in other words, it assumes
> that the pod will run for its full lifecycle.  This represents, for example, batch workloads that do not perform any
> checkpointing.  In the future we will expose configuration options to SimKube to control this behaviour.
