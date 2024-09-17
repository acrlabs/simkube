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

## Configuring your simulation cluster

### Local cluster via kind

This section explains how to create a [`kind`](https://kind.sigs.k8s.io) cluster on your local machine for running
simulations.  If you have a pre-existing Kubernetes cluster that you will be using for your simulation environment, you
can skip this step.

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

## Installation using pre-built images

SimKube images are [hosted on quay.io](https://quay.io/organization/appliedcomputing); the easiest way to install and
run SimKube in your cluster is to use these images along with the provided [kustomize](https://kubernetes.io/docs/tasks/manage-kubernetes-objects/kustomization/)
YAML files in `k8s/kustomize`:

```
git clone https://github.com/acrlabs/simkube && cd simkube
kubectl apply -k k8s/kustomize
```

You should now see the SimKube pods running in your cluster:


```
> kubectl get pods -n simkube
NAMESPACE   NAME                              READY   STATUS      RESTARTS   AGE
simkube     sk-ctrl-depl-b6fbb7744-l8bwm      1/1     Running     0          11h
simkube     sk-tracer-depl-74546ccb48-5gmbc   1/1     Running     0          11h
```

You'll need to also install `skctl` to start or interact with simulations; `skctl` is available on
[crates.io](https://crates.io/crates/skctl) and you can install it with:

```
cargo install skctl
```

You can test if it worked by running `skctl version` (make sure that your Cargo bin directory is on your `$PATH`, e.g.,
`echo "export ${CARGO_HOME}/bin:${PATH}" >> ~/.zshrc`):

```
> skctl version
skctl 1.0.1
```

Now you should be able to [run your first simulation](./running.md)!
