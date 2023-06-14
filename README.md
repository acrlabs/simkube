# simkube

A collection of tools for simulating Kubernetes scheduling and autoscaling behaviour

## Overview

This package provides the following:

- `simkube`: a [Virtual Kubelet](https://virtual-kubelet.io)-based "hollow node" that allows customization based off a
  "skeleton" node file (see the example in `simkube/manifests/skeleton-node-configmap.yml`).

## Developing

It is highly recommended that you install [pre-commit](https://pre-commit.com); this will run useful checks before you
push anything to GitHub.  To set up the hooks in this repo, run `pre-commit install`.

You can develop and test locally against a [kind](https://kind.sigs.k8s.io) cluster.  Follow the steps to [have kind use
a local Docker registry](https://kind.sigs.k8s.io/docs/user/local-registry).  Then, run

```
make build image run
```
