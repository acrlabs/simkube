<!--
template: docs.html
-->

# Contributing to SimKube

## Code organization

The SimKube repo is organized as follows:

```
/<root>
    /build     - build scripts and helper functions (git submodule)
    /docs      - Documentation
    /examples  - Example YAML files and other configs
    /images    - Dockerfiles for all components
    /k8s       - 🔥Config Python scripts for Kubernetes manifest generation
    /sk-api    - CRD and OpenAPI v3 definitions for the SimKube API
    /sk-cli    - Rust code for the `skctl` CLI utility
    /sk-core   - Rust code for shared/"core" functionality
    /sk-ctrl   - Rust code for the `sk-ctrl` Kubernetes controller
    /sk-driver - Rust code for the `sk-driver` Simulation runner
    /sk-store  - Rust code for the tracer object store
    /sk-tracer - Rust code for the `sk-tracer` Kubernetes object
    /testutils - helper functions for tests
```

In general, code that is specific to a single artifact should go in the subdirectory for that artifact, but code that
needs to be shared between multiple artifacts should go in either `lib/`.

> [!NOTE]
> If you are planning to make changes to the API (either the Custom Resource Definition or the SimKube API), please read
> the [API changes](./api_changes.md) document first!

## Building and Deploying

### Building the artifacts

To build all SimKube artifacts for the first time run:

```
git submodule init && git submodule update
make build` from the root of this repository.
```

For all subsequent builds of SimKube artifacts, run only `make build` from the root of this repository.

All build artifacts are placed in the `.build` directory at the root of the repository.  If you just want to build a
subset of the SimKube artifacts, you can set the `ARTIFACTS` environment variable.  This will help limit compilation
time if you are just working on a single or a few components:

```
ARTIFACTS="sk-ctrl sk-driver" make build
```

By default, all the artifacts are built inside Docker containers.  All the intermediate compilation steps and the
executables are saved in `.build/cargo`.

### Building docker images

As you are developing, if you need to deploy your artifacts to a Kubernetes cluster, you will need to build a Docker
image for them.  `make image` is a shorthand for this.  As before, if you just want to build images for a subset of
artifacts, you can limit the scope with the `ARTIFACTS` environment variable.  You can also point the build to your
docker registry by setting the `DOCKER_REGISTRY` environment variable; it defaults to `localhost:5000`.

To accomplish automatic updates of the changed artifacts in Kubernetes (see below), during the image build phase, images
are tagged with a SHA based on the contents of your Git repo, plus a UUID.  🔥Config is smart enough to update the
Deployment manifests with the new SHA after every image build, which means that the artifacts that have changed will
automatically be updated by Kubernetes and everything else will be untouched.

One consequence of the above is that if you don't periodically clean up old built Docker images, you can fill up your
hard drive rather rapidly!  We recommend having `/var` (or wherever your Docker images live) be on a separate partition.
We'd like to improve this situation sometime in the future but it's a low priority right now.

### Running the artifacts in Kubernetes

All Kubernetes manifests are built using [🔥Config](https://github.com/acrlabs/fireconfig), which is a thin wrapper
around [cdk8s](https://cdk8s.io).  To create the manifests and apply them to your Kubernetes cluster, do `make run`.
This will deploy changes to the cluster configured in your current KUBECONFIG.  You must have cluster admin privileges
on that cluster.

All generated Kubernetes manifests live in `.build/manifests`.  All manifests are generated every time (i.e., the
`ARTIFACTS` environment variable has no impact on this stage).  However, only the artifacts that have changed since the
last time you deployed will actually be updated in your cluster.

### Doing everything at once

As a convenience, you can run `make` or `make all` to run all three steps.  The `ARTIFACTS` and `DOCKER_REGISTRY`
environment variables will be respected for the steps where they make sense.

## Linting and running tests

### Testing your changes

Tests are divided into "unit tests" and "integration tests".  The distinction between these is fuzzy.  To run all the
tests, do `make test`.

### Linting your changes

Code linting rules are defined in `.rustfmt.toml`.  We also use [clippy](https://doc.rust-lang.org/stable/clippy/usage.html)
for additional Rust linting and checks.  We use a _nightly_ version of rustfmt to take advantage of unstable formatting
rules, so you will need to install a nightly toolchain here.  (Note that all actual Rust code does not use any nightly
features).  You can run all lints with `make lint`.

### Code coverage

We don't require 100% test coverage for this project, and don't have a set threshold that is needed for tests.  However,
all functionality "of consequence" should be tested.  If you're writing a new feature and you think it's "of
consequence", please also write tests for them (see [Writing new tests](#writing-new-tests), below).

Code coverage is checked whenever you open a PR using [CodeCov](https://about.codecov.io).  It will leave a helpful
comment showing you coverage changes and provide a link to a dashboard with more detailed information about what is and
is not covered.

If you'd like to generate coverage reports locally, simply set the `WITH_COVERAGE` variable:

```
WITH_COVERAGE=1 make test
```

### Writing new tests

There is a suite of utility functions for tests in `/testutils` that provide additional fixtures and helper
functions for writing tests.  Feel free to add more utilities in here if it would be helpful.

Tests should in most cases be put into a separate submodule called `tests` and included with a

```rust
#[cfg(test)]
mod tests;
```

block at the bottom of the main module.

Tests in a separate test directory are automatically excluded from code coverage metrics.  If instead the test is
colocated with the source code, you can exclude it from code coverage metrics with the following block:

```rust
#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests {
    ...
}
```

### Integration tests

We don't use the native rust/Cargo integration tests because we want to be able to assert on internal state in an
integration test, which native Rust integration tests don't allow.  Instead, integration tests should be placed in a
separate `itest` submodule within the `tests` submodule; they will be automatically run separately from the unit tests.
There's no hard-and-fast rule as to what an integration test is versus a unit test, but in general if it's testing a
"complete set" of functionality or a feature, it's probably an integration test.  All snapshot (`cargo insta`) tests
should be integration tests.

## Making a PR

> [!WARNING]
> Due to the uncertain nature of copyright and IP law, this repository does not accept contributions that have been all
> or partially generated with GitHub Copilot or other LLM-based code generation tools.  Please disable any such tools
> before authoring changes to this project.

Follow the standard process for opening a pull request.  Please provide a nice description of your contribution.  GitHub
actions will run tests, linting, and build.  If there are any errors there, please fix them!  Once your tests are
passing, one of the maintainers will sign off on the PR and merge it to master.
