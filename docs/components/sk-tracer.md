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
    podSpecTemplatePaths: /json/patch/path/to/pod/template/spec
    trackLifecycle: true/false (optional)
```

Here is an example config file that watches both Deployments and VolcanoJobs from the [Volcano](https://volcano.sh/en/)
Kubernetes scheduler:

```yaml
trackedObjects:
  apps/v1.Deployment:
    podSpecTemplatePaths: /spec/template
  batch.volcano.sh/v1alpha1.Job:
    podSpecTemplatePaths: /spec/tasks/*/template
    trackLifecycle: true
```

The `podSpecTemplatePaths` field uses a non-standard extension to the [JSONPatch](https://jsonpatch.com) specification.
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
configuration.  The API for this is defined in [`api/v1/simkube.yml`](https://github.com/acrlabs/simkube/blob/main/sk-api/schema/v1/simkube.yml).  The
response from the tracer will be a bytestream of the trace stored in the [SimKube trace format](../ref/trace-files.md).

Some initial cleaning of the PodSpec is done to remove objects that can change on each deployment.  The goal/idea is
that this should be a stable and reproducible hash in the simulated cluster.
