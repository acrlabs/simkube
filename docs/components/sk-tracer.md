<!--
template: docs.html
-->

# SimKube Tracer

The SimKube Tracer runs in a production Kubernetes cluster and watches all configured resources, as well as all pods
owned by those resources, and records changes to these objects in an in-memory trace.  On-demand, the tracer will export
all or a portion of the trace to persistent storage so that it can be replayed later.

## Usage

```bash exec="on" result="plain"
sk-tracer --help
```

## Config File Format

```yaml
trackedObjects:
  <gvk for object>:
    podSpecTemplatePath: /json/patch/path/to/pod/template/spec
    trackLifecycle: true/false (optional)
```

Here is an example config file that watches both Deployments and VolcanoJobs from the [Volcano](https://volcano.sh/en/)
Kubernetes scheduler:

```yaml
trackedObjects:
  apps/v1.Deployment:
    podSpecTemplatePath: /spec/template
  batch.volcano.sh/v1alpha1.Job:
    podSpecTemplatePath: /spec/tasks/*/template
    trackLifecycle: true
```

The `podSpecTemplatePath` field uses a non-standard extension to the [JSONPatch](https://jsonpatch.com) specification.
Specifically, if the value at a particular location in the patch path is an array, you can specify `*` to indicate that
it should apply to all elements of the array.  It is a type error if the value is not an array.

This extension is necessary because the tracer modifies the pod template spec before it is saved in the trace, and some
resources (for example, the VolcanoJob mentioned above) allow the specification of multiple pod templates.

## Details

The SimKube Tracer establishes a watch on the Kubernetes apiserver for all resources mentioned in the config file.
Because these are dynamically determined at runtime, the tracer must use the unstructured API for this purpose.  The
tracer _also_ establishes a watch on all pods in the cluster.  Whenever a new pod is created, the tracer walks the
ownership chain to determine if any of the pod's ancestors are being tracked.  If so, _and_ if the `trackLifecycle`
flags is set for that owner, the tracer will record the pod lifecycle events (currently just start and end timestamps)
in the trace for use by the simulator.

## Exporting a trace

A user can export a trace by making a post request to the `/export` endpoint and including a JSON object with the export
configuration.  The API for this is defined in
[`api/v1/simkube.yml`](https://github.com/acrlabs/blob/master/api/v1/simkube.yml).  The response from the tracer will be
a bytestream of the trace stored in [msgpack](https://msgpack.org) format, which is a JSON-like binary
format.  You can inspect the contents of the trace with the `msgpack2json` utility from
[msgpack-tools](https://github.com/ludocode/msgpack-tools):

```
msgpack2json -di /path/to/trace/file
```

The structure of the trace file is a 4-tuple of data:

```
[
    {tracer config},
    [timeseries data of "important" events],
    {index of tracked objects during the course of the trace},
    {pod lifecycle data for tracked pods},
]
```

An entry in the timeseries array looks like this:

```yaml
{
    ts: <unix timestamp>,
    applied_objs: [array of Kubernetes object definitions],
    deleted_objs: [array of Kubernetes object definitions],
}
```

The "tracked object index" (the third entry in the trace) stores the namespaced name of the object along with a hash of
the object contents.  The pod lifecycle data has the following format:

```yaml
{
    <pod owner's namespaced name>: {
        <pod hash>: [{start_ts: <unix timestamp>, end_ts: <unix timestamp>}, ...]
        ...
    },
}
```

Because pods in the simulation will not have the same names as in the production trace, we can't use the pod name as a
stable identifier to track lifecycles.  So instead, we index by the pod owner, and the hash of the pod object.  Because
an owner can have pods with different characteristics (e.g., if a Deployment changes and creates a new ReplicaSet, or if
there are multiple pod types specified in a VolcanoJob), we must track the lifecycles for these pods separately.  This
is done by way of the hash of the PodSpec.

Some initial cleaning of the PodSpec is done to remove objects that can change on each deployment.  The goal/idea is
that this should be a stable and reproducible hash in the simulated cluster.
