<!--
template: docs.html
-->
# SimKube Amazon Machine Images (AMIs)

Simkube provides prebuilt Amazon Machine Images (AMIs) for running simulations in AWS without having to install or configure SimKube manually.

Our AMIs are intended for users who want a repeatable, preconfigured simulation environment for SimKube.

## What the AMI is for

The SimKube AMI is designed for:

- running SimKube simulations on EC2
- providing a consistent environment across runs
- reducing setup and dependency management
- running larger, longer simulations than you can run locally with SimKube
- enabling SimKube to be run in CI pipelines

## Available AMIs

- SimKube AMI
Suitable for running on demand SimKube workloads and long running simulations.
- SimKube GitHub Runner AMI
Designed specifically with CI in mind. Use this AMI as an ephemeral SimKube GitHub Action Runner.

## What's included

Feature | SimKube AMI | SimKube GitHub Runner AMI
:--- | :--- | :--- |
Ubuntu 24.04 LTS Operating System | ✅ | ✅
All SimKube components* | ✅ | ✅
All SimKube dependencies | ✅ | ✅
container runtime and required system dependencies | ✅ | ✅
For our `SimKube GitHub Runner AMI` the GitHub Actions Runner software | ❌ | ✅

Our AMIs are optimized for running simulations, and are not recommended for any other use cases.

\* more on SimKube components [here](../components/sk-ctrl.md)

## Next steps

- [Launch and use SimKube AMIs](./usage.md)
- [Configure GitHub Actions to run simulations on self-hosted runners](github-runners.md)
