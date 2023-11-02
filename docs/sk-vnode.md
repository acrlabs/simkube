# SimKube Virtual Node

The SimKube Virtual Node is build on top of [Virtual Kubelet](https://virtual-kubelet.io) to present a simulated/virtual
node in a cluster.

## Usage

```
Run a simulated Kubernetes node

Usage:
  sk-vnode [flags]

Flags:
  -h, --help                   help for sk-vnode
      --jsonlogs               structured JSON logging output
  -n, --node-skeleton string   location of config file (default "node.yml")
  -v, --verbosity int          log level output (higher is more verbose (default 2)
```

## Details

The virtual node accepts all pods that are assigned to them, assuming the pass all the standard Kubelet checks (e.g. all
node selectors match, etc.)  The pod and all containers in it will instantly be marked as Running.  If the [pod
lifecycle annotations](#pod-lifecycle-annotations) are set, the pod will terminate all running containers after the
specified time and report that the Pod succeeded.

### Node Configuration

The virtual node can be configured to look like any node in a real Kubernetes cluster.  Users provide the virtual node
with a node skeleton file that contains all of the properties that node should have.  This can be generated from a real
node object by running `kubectl get -o yaml node test-worker` and modifying accordingly.  Here is an example node
skeleton that presents as a node with 32GB of RAM and 16 CPUs:

```
apiVersion: v1
kind: Node
status:
  allocatable:
    cpu: "16"
    memory: "32Gi"
  capacity:
    cpu: "16"
    memory: "32Gi"
```

### Pod Lifecycle Annotations

If the incoming pod has a `simkube.io/lifetime-seconds: XX` annotation on it, then the virtual node will run the pod for
only `XX` seconds before terminating all running containers and marking the pod as successful.
