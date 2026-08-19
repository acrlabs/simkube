<!--
template: docs.html
-->

# SimKube Trace File Format

The SimKube trace file format is a structured object stored as a [msgpack](https://msgpack.org) file, which is a
JSON-like binary format.  You can inspect the contents of the trace using [`skctl xray`](../components/skctl.md) or
with the `msgpack2json` utility from [msgpack-tools](https://github.com/ludocode/msgpack-tools):

```text
skctl xray /path/to/trace/file
```

or

```text
msgpack2json -di /path/to/trace/file
```

## Trace File Schema

The structure of the trace file is a map with the following schema; of data; all entries are (currently) required:

```text
{
    "version": 3,
    "config": {...},
    "events": [...],
    "index": {...},
}
```

### Version

Modern versions of SimKube (v2+) require a "version" field specified in the trace file.  This tells SimKube how to parse
the remainder of the file, and SimKube will panic if the field is not present.  The current trace file format version is
`3`.

### Config

The `sk-tracer` [sk-tracer config file](../components/sk-tracer.md) is stored alongside the events in the trace file.

### Events

An entry in the timeseries array looks like this:

```text
{
    ts: <unix timestamp>,
    applied_objs: [array of Kubernetes object definitions],
    deleted_objs: [array of Kubernetes object definitions],
}
```

### Index

The "index" (the third entry in the trace) stores an index of "owning" objects in the trace, together with metadata
about the pods that belong to those objects.  The format is:

```text
<GVK>: {
    <object 1's namespaced name>: {
        mtime1: [pod_sim_datas...]
        mtime2: [pod_sim_datas...]
    },
    <object 2's namespaced name>: {
        mtime3: [pod_sim_datas...]
        mtime4: [pod_sim_datas...]
    },
    ...
}
...
```

The `mtime` values are all the timestamps at which the owning object has been updated in the trace (they correspond
exactly with the `ts` field in the event entry).  The array of `pod_sim_datas` is a list of pod metadata for pods that
belong to the owning object within the specified time window (e.g., between `mtime1` and `mtime2`).  The `pod_sim_data`
struct currently stores pod lifecycle information:

```text
{
    lifecycle: {"Finished": [<pod start timestamp>, <pod end timestamp>]},
}
```

Because pods in the simulation will not have the same names as in the production trace, we can't use the pod name as a
stable identifier to track lifecycles.  So instead, we index by the pod owner, and the last modification time of owning
resource.  This allows SimKube to track changes in pod behaviour across changes to the owning resource (a simple example
is a CronJob that changes `sleep 60` to `sleep 120`).

> [!NOTE]
> Previous versions of SimKube used a "stable" hash of the pod spec to tie running pods back to their owning resources;
> however, this only works in extremely specialized circumstances, and has since been changed.  Note also that previous
> versions of SimKube were purportedly able to disambiguate "different types" of pods that belong to the same owning
> resource; this also never worked well, but we may make another attempt to support this in the future.
