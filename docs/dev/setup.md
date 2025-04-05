<!--
template: docs.html
-->

# Setting up your SimKube development environment

This guide will walk you through the steps to build and install SimKube from source.  You'll need to have a pre-existing
Kubernetes cluster to install SimKube on (follow the [steps](../intro/installation.md) to set up a local cluster with
Kind).

## Prerequisites

In addition to the project prerequisites, you will need to have the following installed:

- [pre-commit](https://pre-commit.com)
- Nightly version of rustfmt
- [cargo-nextext](https://nexte.st) for running tests
- Nightly version of [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) for running tests
- [cargo-insta](https://insta.rs/docs/quickstart/) (only needed for updating/adding new snapshot tests)

SimKube uses [🔥Config](https://github.com/acrlabs/fireconfig) to generate Kubernetes manifests from definitions located
in `./k8s/`.  If you want to make changes to the generated Kubernetes manifests, you will need to install the
following additional dependencies:

- Python 3.11
- Python Poetry (https://python-poetry.org/docs/)
- NodeJS

### Optional prerequisites

- [grcov](https://github.com/mozilla/grcov) (if you want to generate coverage reports locally)
- [openapi-generator](https://openapi-generator.tech) (if you need to make changes to the SimKube API)
- [msgpack-tools](https://github.com/ludocode/msgpack-tools) (for inspecting the contents of exported trace files)

### Setup

Run `make setup` to install the pre-commit hooks and configure the Poetry virtualenv in `./k8s`

## Building SimKube

To build all SimKube artifacts for the first time run:

```
git clone https://github.com/acrlabs/simkube && cd simkube
git submodule init && git submodule update
make build
```

For all subsequent builds of SimKube artifacts, run only `make build` from the root of this repository.

## Docker images

To build and push Docker images for all the artifacts, run `DOCKER_REGISTRY=path_to_your_registry:5000 make image`

## Running the artifacts:

To run the artifacts using the images you built in the previous step, run `make run`.   You should now see the SimKube
pods running in the `simkube` namespace on your Kubernetes cluster:

```
> kubectl get pods -n simkube
NAMESPACE   NAME                              READY   STATUS      RESTARTS   AGE
simkube     sk-ctrl-depl-b6fbb7744-l8bwm      1/1     Running     0          11h
simkube     sk-tracer-depl-74546ccb48-5gmbc   1/1     Running     0          11h
```

## Cleaning up

All build artifacts are placed in the `.build/` directory.  You can remove this directory or run `make clean` to clean
up.
