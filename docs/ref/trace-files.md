<!--
template: docs.html
-->

# SimKube Trace File Format

The SimKube trace file format is a structured object stored as a [msgpack](https://msgpack.org) file, which is a
JSON-like binary format.  You can inspect the contents of the trace using [`skctl xray`](../components/skctl.md) or
with the `msgpack2json` utility from [msgpack-tools](https://github.com/ludocode/msgpack-tools):

```
skctl xray /path/to/trace/file
```

or


```
msgpack2json -di /path/to/trace/file
```

## Trace File Schema

The structure of the trace file is a map with the following schema; of data; all entries are (currently) required:


```
{
    "version": 2,
    "config": {...},
    "events": [...],
    "index": {...},
    "pod_lifecycles": {...},
}
```

### Version

Modern versions of SimKube (v2+) require a "version" field specified in the trace file.  This tells SimKube how to parse
the remainder of the file, and SimKube will panic if the field is not present.  The current trace file format version is
`2`.

### Config

The `sk-tracer` [sk-tracer config file](../components/sk-tracer.md) is stored alongside the events in the trace file.

### Events

An entry in the timeseries array looks like this:

```
{
    ts: <unix timestamp>,
    applied_objs: [array of Kubernetes object definitions],
    deleted_objs: [array of Kubernetes object definitions],
}
```

### Index

The "index" (the third entry in the trace) stores the namespaced name of the object along with a hash of the object contents:

```
<GVK>: {
    <object 1's  namespaced name>: <object manifest hash>
    <object 2's  namespaced name>: <object manifest hash>
    ...
}
...
```
### Pod lifecycles

The pod lifecycle data has the following format (the key to each entry is a 2-tuple of the pod owner's GVK as well as
the pod owner's namespaced name):

```
{
    (<pod owner's GVK>, <pod owner's namespaced name): {
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

Note that the pod lifecycles field uses a few capabilities that are supported by msgpack, but not by JSON: namely, some
msgpack libraries (notably, `python-msgpack`) will not parse the 2-tuple map key, and JSON does not support integer map
keys for the pod hash.  These incompatibilities make round-tripping between msgpack and JSON difficult.
