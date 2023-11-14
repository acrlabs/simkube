---
project: SimKube
template: docs.html
---

# SimKube Virtual Cloud Provider

SimKube uses the Cluster Autoscaler
[externalgrpc](https://github.com/kubernetes/autoscaler/tree/master/cluster-autoscaler/cloudprovider/externalgrpc) cloud
provider to scale the virtual nodes in the cluster up and down.

## Usage

```
gRPC cloud provider for simkube

Usage:
  sk-cloudprov [flags]

Flags:
  -A, --applabel string   app label selector for virtual nodes (default "sk-vnode")
  -h, --help              help for sk-cloudprov
      --jsonlogs          structured JSON logging output
  -v, --verbosity int     log level output (higher is more verbose (default 2)
```

## Details

The SimKube Cloud Provider implements 90% of the interface described
[here](https://github.com/kubernetes/autoscaler/blob/master/cluster-autoscaler/cloudprovider/externalgrpc/protos/externalgrpc.proto).
Currently the only missing functionality that may be important someday is the `NodeGroupTemplateNodeInfo`, used when
scaling up from 0.

When scaling up, the cloud provider simply increases the size of the virtual node deployment.  Cluster Autoscaler needs
to select specific nodes for termination during scale-down, and this is accomplished using the [pod deletion
cost](https://kubernetes.io/docs/concepts/workloads/controllers/replicaset/#pod-deletion-cost) feature of the ReplicaSet
controller.

The cloud provider gRPC server listens on port 8086.
