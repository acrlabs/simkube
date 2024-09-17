<!--
project: SimKube
template: docs.html
-->

# Installing SimKube

This guide will walk you through installing the various SimKube components

## Prerequisites

The following prereqs are required for all components:

- [Rust (including Cargo)](https://www.rust-lang.org/learn/get-started) >= 1.71
- [Docker](https://docs.docker.com/get-started/)
- [kubectl](https://kubernetes.io/docs/tasks/tools/) >= 1.27
- a Kubernetes cluster running at least v1.27

Additional prerequisites are necessary for your simulation cluster:

- [KWOK](https://kwok.sigs.k8s.io) >= 0.4.0
- [CertManager](https://cert-manager.io) for setting up mutating webhook certificates

### Optional Prerequisites

If you want to run SimKube on a local development cluster, [kind](https://kind.sigs.k8s.io) >= 0.19 is the supported
tooling for doing so.

If you want to test autoscaling, SimKube currently supports either the [Kubernetes Cluster Autoscaler](https://github.com/kubernetes/autoscaler)
or [Karpenter](https://karpenter.sh).  You will need to install and configure these applications to use the
corresponding KWOK provider.  For the Kubernetes Cluster Autoscaler, a KWOK [cloud provider](https://github.com/kubernetes/autoscaler/tree/master/cluster-autoscaler/cloudprovider/kwok)
is available, and for Karpenter, a basic [KWOK provider](https://github.com/kubernetes-sigs/karpenter/tree/main/kwok) is
used.  See [Autoscaling](../adv/autoscaling.md) for more information on configuring these tools.

## Installation using pre-built images (for users)

SimKube images are [hosted on quay.io](https://quay.io/organization/appliedcomputing); the easiest way to install and
run SimKube in your cluster is to use these images along with the provided [kustomize](https://kubernetes.io/docs/tasks/manage-kubernetes-objects/kustomization/)
YAML files in `k8s/kustomize`:

```
git clone https://github.com/acrlabs/simkube && cd simkube
kubectl apply -k k8s/kustomize
```

You'll need to also install `skctl` to start or interact with simulations; `skctl` is available on
[crates.io](https://crates.io/crates/skctl) and you can install it with:

```
cargo install skctl
```

## Installation from source (for SimKube developers)

If you instead want to build and install SimKube from source, you can follow these steps:

### Building SimKube

To build all SimKube artifacts for the first time run:

```
git clone https://github.com/acrlabs/simkube && cd simkube
git submodule init && git submodule update
make build
```

For all subsequent builds of SimKube artifacts, run only `make build` from the root of this repository.

### Docker images

To build and push Docker images for all the artifacts, run `DOCKER_REGISTRY=path_to_your_registry:5000 make image`

### Running the artifacts:

To run the artifacts using the images you built in the previous step, run `make run`.   You should now see the SimKube
pods running in the `simkube` namespace on your Kubernetes cluster:

```
> kubectl get pods -n simkube
NAMESPACE   NAME                              READY   STATUS      RESTARTS   AGE
simkube     sk-ctrl-depl-b6fbb7744-l8bwm      1/1     Running     0          11h
simkube     sk-tracer-depl-74546ccb48-5gmbc   1/1     Running     0          11h
```

### Cleaning up

All build artifacts are placed in the `.build/` directory.  You can remove this directory or run `make clean` to clean
up.

## Configuring your simulation cluster

### Local cluster via kind

This section explains how to create a [`kind`](https://kind.sigs.k8s.io) cluster on your local machine for running simulations.
If you have a pre-existing Kubernetes cluster that you will be using for your simulation environment, you can skip this
step.

From the `kind` website:

> kind is a tool for running local Kubernetes clusters using Docker container “nodes”.  kind was primarily designed for
> testing Kubernetes itself, but may be used for local development or CI.

You'll want to create a kind cluster with at least two nodes.  Here's an example configuration:

```yaml
# kind.yml
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
  - role: control-plane
    labels:
      type: kind-control-plane
  - role: worker
    labels:
      type: kind-worker
```

If you are pushing the SimKube docker images to a local docker registry, you will additionally need to follow the steps
in [Create A Cluster and Registry](https://kind.sigs.k8s.io/docs/user/local-registry/) to enable `kind` to access your
images.

Create the cluster by running

```
> kind create cluster --name simkube --config kind.yml
```

### Install Required Dependencies

**KWOK**:

```
> KWOK_REPO=kubernetes-sigs/kwok
> KWOK_LATEST_RELEASE=$(curl "https://api.github.com/repos/${KWOK_REPO}/releases/latest" | jq -r '.tag_name')
> kubectl apply -f "https://github.com/${KWOK_REPO}/releases/download/${KWOK_LATEST_RELEASE}/kwok.yaml"
> kubectl apply -f "https://github.com/${KWOK_REPO}/releases/download/${KWOK_LATEST_RELEASE}/stage-fast.yaml"
```

**Prometheus Operator**:

```
> git clone https://github.com/prometheus-operator/kube-prometheus.git
> cd kube-prometheus
> kubectl create -f manifests/setup
> until kubectl get servicemonitors --all-namespaces ; do date; sleep 1; echo ""; done
No resources found  # this message is expected
> kubectl create -f manifests/
```

**cert-manager**:

```
> kubectl apply -f https://github.com/cert-manager/cert-manager/releases/download/v1.14.3/cert-manager.yaml
> kubectl wait --for=condition=Ready -l app=webhook -n cert-manager pod --timeout=60s
```

We're going to use self-signed certificates for this, so apply the following file to your cluster:

```yaml
# self-signed.yml
apiVersion: cert-manager.io/v1
kind: ClusterIssuer
metadata:
  name: selfsigned
  namespace: kube-system
spec:
  selfSigned: {}
```

```
> kubectl apply -f self-signed.yml
```

## Customizing SimKube

After completing the above steps, you should have a basic SimKube installation running on your Kubernetes cluster!
The following section describes some options for customizing the behaviour of your SimKube installation; if you are
using the provided [kustomize](https://github.com/acrlabs/simkube/tree/master/k8s/kustomize) manifests, you can update
or override these values there.

### Configuration `sk-tracer`

The SimKube tracer runs in a real cluster and collects data about changes to objects in that cluster.  You can configure
what objects it watches via a config file, which is injected into the `sk-tracer` pod as a ConfigMap; if you are using
the provided kustomize manifests, you can override the `tracer-config.yml` data in the provided ConfigMap.  Here is an
example config that tells sk-tracer to watch Deployments, Jobs, and StatefulSets:

```yaml
trackedObjects:
  apps/v1.Deployment:
    podSpecTemplatePath: /spec/template
  batch/v1.Job:
    podSpecTemplatePath: /spec/template
  apps/v1.StatefulSet:
    podSpecTemplatePath: /spec/template
```

> [!NOTE]
> SimKube does some sanitization of the resources it watches, which is why it needs to know where the
> `podSpecTemplatePath` is; especially for custom resources, the path to the `podSpecTemplate` is not necessarily
> standard or well-known.  In a future version of SimKube we'll make this parameter optional for all "standard"
> Kubernetes objects.

`sk-tracer` needs an RBAC policy that grants "get", "list" and "watch" access to all configured objects in the cluster,
as well as pods.  For example, if you use the above configuration, you will need the following RBAC policy attached to
the service account used by `sk-tracer`:

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: sk-tracer
rules:
- apiGroups: [""]
  resources: ["pods"]
  verbs: ["get", "watch", "list"]
- apiGroups: ["apps/v1"]
  resources: ["deployment", "statefulset"]
  verbs: ["get", "watch", "list"]
- apiGroups: ["batch/v1"]
  resources: ["job"]
  verbs: ["get", "watch", "list"]
```

### Configuring `sk-ctrl`

The SimKube controller just needs the SimKube custom resources installed in the target environment, and needs no other
configuration.

The SimKube controller needs, at a minimum, write access for all of the objects that it will be simulating.  In theory,
since this is an isolated (or potentially even local) environment, it should be safe to give it `cluster-admin`, which
is probably the easiest way to configure it.
