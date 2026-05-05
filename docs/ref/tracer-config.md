<!--
template: docs.html
-->

# SimKube Tracer Config Schema

The SimKube tracer config file format has a single top-level field called `trackedObjects`, which is a map from a set of
GVK strings that the tracer should watch to `TrackedObjectConfig` objects.  The string format for the GVK is
`group/version.kind`, for example, `apps/v1.Deployment`.  The `TrackedObjectConfig` object has the following schema:

<div class="schema" markdown>
/// details | `TrackedObjectConfig`
Optional configuration for what types of tracked objects the tracer should watch.

/// details | `podSpecTemplatePaths`: `[]string`
A list of JSON pointers to the location of the `podSpecTemplate` field(s) in the resource.
///

/// details | `trackLifecycle`: `bool`
Set to `true` if the tracer should track details about lifecyles (start/stop times, etc) for pods owned by this
resource; defaults to `false`.
///

/// details | `skipOwned`: `bool`
Set to `true` if the tracer should ignore _any_ owned variant of this resource; by default, the tracer will only ignore
an object if its owner is _also_ tracked by the tracer.
///

///
</div>

If no configuration is needed for a particular resource type, you _must_ still specify an empty dictionary object using
`{}` notation.

## Example

Here is an example config file that watches both Deployments and VolcanoJobs from the [Volcano](https://volcano.sh/en/)
Kubernetes scheduler:

```yaml
trackedObjects:
  apps/v1.Deployment: {}
  batch.volcano.sh/v1alpha1.Job:
    podSpecTemplatePaths:
      - /spec/tasks/*/template
    trackLifecycle: true
```

## Built-in `podSpecTemplatePaths`

A number of "standard" Kubernetes objects have the `podSpecTemplatePaths` field defined in the tracer itself and do not
need to be specified in the config; for backwards compatibility, users _can_ still specify these values in the config,
but it is an error if the specified value does not match the hard-coded value in the tracer.  The current list of
resources with pre-defined `podSpecTemplatePaths` is:

- batch/v1.CronJob
- apps/v1.DaemonSet
- apps/v1.Deployment
- batch/v1.Job
- apps/v1.ReplicaSet
- apps/v1.StatefulSet

## JSON Pointer Extension

The `podSpecTemplatePaths` entries use a non-standard extension to the [JSON Pointer](https://datatracker.ietf.org/doc/html/rfc6901)
specification: any path segments that refer to array objects can contain a wildcard `*` to match all entries in that
array, as you can see in the example above.
