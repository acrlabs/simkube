![build status](https://github.com/acrlabs/simkube/actions/workflows/verify.yml/badge.svg)

# simkube

A collection of tools for simulating Kubernetes scheduling and autoscaling behaviour

## Overview

This package provides the following components:

- `sk-cloudprov`: an [external gRPC-based cloud provider](https://github.com/kubernetes/autoscaler/tree/master/cluster-autoscaler/cloudprovider/externalgrpc)
  for Cluster Autoscaler that can communicate with and scale the `sk-vnode` "node group".
- `sk-ctrl`: a Kubernetes Controller that watches for Simulation custom resources and runs a simulation based on the
  provided trace file.
- `sk-driver`: the actual runner for a specific simulation, created as a Kubernetes Job by `sk-ctrl`
- `sk-tracer`: a watcher for Kubernetes pod creation and deletion events, saves these events in a replayable trace
  format.
- `sk-vnode`: a [Virtual Kubelet](https://virtual-kubelet.io)-based "hollow node" that allows customization based off a
  "skeleton" node file (see the example in `simkube/manifests/dist/0000-sk-vnode.k8s.yaml`)

## Developing

When you first clone the repository, run `make setup`; this will initialize [pre-commit](https://pre-commit.com) and the
Poetry virtualenv for generating the Kubernetes manifests, and vendor in all of the Rust dependencies for faster build
times.

To deploy all the subcomponents, run `make`.  This will also create a test deployment which is scheduled on the
virtual nodes.  If you scale the test deployment up or down, Cluster Autoscaler and `sk-cloudprov` will react to scale
the `sk-vnode` deployment object.

If you want to run linting checks and tests manually, you can run `make verify`.  Since this repo includes both Go and
Rust code, the linting results, test results, and code coverage will be split for the two languages, which is slightly
annoying.

Kubernetes manifests are generated via [🔥Config](https://github.com/acrlabs/fireconfig), which is based on top of
[cdk8s](https://cdk8s.io).  You don't _have_ to use the generated manifests but they are generally recommended.

All relevant build artifacts get placed in `.build`.  If you'd like to remove them you can run `make clean`.
