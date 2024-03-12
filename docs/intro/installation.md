<!--
project: SimKube
template: docs.html
-->

# Installing SimKube

This guide will walk you through installing the various SimKube components

## Prerequisites

The following prereqs are required for all components:

- Rust >= 1.71
- Docker
- kubectl >= 1.27
- Kubernetes >= 1.27

Additional prerequisites are necessary for your simulation cluster:

- [KWOK](https://kwok.sigs.k8s.io) >= 0.4.0
- [CertManager](https://cert-manager.io) for setting up mutating webhook certificates

### Optional Prerequisites

SimKube uses [🔥Config](https://github.com/acrlabs/fireconfig) to generate Kubernetes manifests from definitions located
in `./k8s/`.  If you want to use this mechanism for generating Kubernetes manifests, you will need to install the
following additional dependencies:

- Python 3.10
- Python Poetry (https://python-poetry.org/docs/)
- NodeJS

Additionally, if you want to run SimKube on a local development cluster, [kind](https://kind.sigs.k8s.io) >= 0.19 is the
supported tooling for doing so.

If you want to test autoscaling, SimKube currently supports either the [Kubernetes Cluster Autoscaler](https://github.com/kubernetes/autoscaler)
or [Karpenter](https://karpenter.sh).  You will need to install and configure these applications to use the
corresponding KWOK provider.  For the Kubernetes Cluster Autoscaler, a KWOK [cloud provider](https://github.com/kubernetes/autoscaler/tree/master/cluster-autoscaler/cloudprovider/kwok)
is available, and for Karpenter, a basic [KWOK provider](https://github.com/kubernetes-sigs/karpenter/tree/main/kwok) is
used.  See [Autoscaling](../adv/autoscaling.md) for more information on configuring these tools.

## Building SimKube

To build all SimKube artifacts for the first time run:

- `git submodule init && git submodule update`
- `make build` from the root of this repository.

For all subsequent builds of SimKube artifacts, run only `make build` from the root of this repository.

### Docker images

To build and push Docker images for all the artifacts, run `DOCKER_REGISTRY=path_to_your_registry:5000 make image`

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

You'll want to create a kind cluster with at least two nodes; moreover, you need to provide the cluster with access to a
directory on your local hard drive from which it can read trace files.  Here's an example configuration:

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
    extraMounts:
      - hostPath: /tmp/kind-node-data
        containerPath: /data
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

## Deploying SimKube

### Generating Kubernetes Manifests

SimKube currently uses [🔥Config](https://github.com/acrlabs/fireconfig) to generate Kubernetes manifests.  You can
generate the required manifests for all SimKube components by running `make k8s` from the root of this repository.

> [!NOTE]
> 🔥Config uses [cdk8s](https://cdk8s.io) internally, which (unfortunately) runs a NodeJS subprocess.  If you don't have
> NodeJS and/or Poetry installed, you can generate the manifests inside a Docker container (thanks
> [@vsoch](https://github.com/vsoch)!):<br>
>
> `> docker run -it --entrypoint bash -v $PWD/:/code node:bookworm`<br>
> `> apt-get update && apt-get install -y python3-poetry`<br>
> `> make k8s`<br>

<br>

> [!WARNING]
> The generated manifests are fairly primitive right now and you may need to customize them in order to get them to
> install in your environment.  We also don't (currently) have any Helm charts available, but we'd welcome a
> contribution!  (See this [GitHub issue](https://github.com/acrlabs/simkube/issues/97))

### Running `sk-tracer`

The SimKube tracer runs in a real cluster and collects data about changes to objects in that cluster.  You can configure
what objects it watches via a config file.  Here is an example config file you can use to watch changes to Deployments,
Jobs, and StatefulSets:

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

To install `sk-tracer` into your cluster, run

```
> kubectl apply -f .build/manifests/0000-global.k8s.yaml
> kubectl apply -f .build/manifests/0001-sk-tracer.k8s.yaml
```

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

### Running `sk-ctrl`

The SimKube controller just needs the SimKube custom resources installed in the target environment, and needs no other
configuration.  After running `make k8s`, run the following commands to install everything:

```
> kubectl apply -f .build/manifests/raw
> kubectl apply -f .build/manifests/0000-global.k8s.yaml
> kubectl apply -f .build/manifests/0002-sk-ctrl.k8s.yaml
```

The SimKube controller needs, at a minimum, write access for all of the objects that it will be simulating.  In theory,
since this is an isolated (or potentially even local) environment, it should be safe to give it `cluster-admin`, which
is probably the easiest way to configure it.
