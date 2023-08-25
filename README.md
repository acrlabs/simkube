# simkube

A collection of tools for simulating Kubernetes scheduling and autoscaling behaviour

## Overview

This package provides the following:

- `sk-vnode`: a [Virtual Kubelet](https://virtual-kubelet.io)-based "hollow node" that allows customization based off a
  "skeleton" node file (see the example in `simkube/manifests/dist/0000-sk-vnode.k8s.yaml`)
- `sk-cloudprov`: an [external gRPC-based cloud provider](https://github.com/kubernetes/autoscaler/tree/master/cluster-autoscaler/cloudprovider/externalgrpc)
  for Cluster Autoscaler that can communicate with and scale the `sk-vnode` "node group".  An example configuration
  for `sk-cloudprov` and Cluster Autoscaler can be found in `simkube/manifests/dist/0002-sk-cloudprov.k8s.yaml` and
  `simkube/manifests/dist/0003-cluster-autoscaler.k8s.yaml`.

## Monitoring

We use the [kube-prometheus](https://github.com/prometheus-operator/kube-prometheus/tree/main) stack to set up
prometheus and grafana for monitoring and data collection.  You need to install `jsonnet`, using your system package
manager or otherwise.

## Developing

When you first clone the repository, run `make setup`; this will initialize [pre-commit](https://pre-commit.com) and the
Poetry virtualenv for generating the Kubernetes manifests.  You will also need to install
[go-carpet](https://github.com/msoap/go-carpet) 1.11.0 or higher:

```
go install https://github.com/msoap/go-carpet@latest
```

To deploy `sk-vnode` and `sk-cloudprov`, run `make`.  This will also create a test deployment which is scheduled on the
virtual nodes.  If you scale the test deployment up or down, Cluster Autoscaler and sk-cloudprov will react to scale the
`sk-vnode` deployment object.

If you want to run linting checks and tests manually, you can run `make test`.

Kubernetes manifests are generated via [🔥Config](https://github.com/acrlabs/fireconfig), which is based on top of
[cdk8s](https://cdk8s.io).  You don't _have_ to use the generated manifests but they are generally recommends.

All relevant build artifacts get placed in `.build`.  If you'd like to remove them you can run `make clean`.
