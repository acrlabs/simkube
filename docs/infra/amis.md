<!--
template: docs.html
-->
# SimKube Amazon Machine Images (AMIs)
[Applied Computing Research Labs](https://appliedcomputing.io) provides prebuilt Amazon Machine Images (AMIs) for running simulations in AWS without having to install or configure SimKube manually.

Our AMIs are intended for users who want a repeatable, preconfigured simulation environment for SimKube.

## Quick Start Guides
- [Run SimKube in AWS EC2](./run-sim.md)
- [Run SimKube in CI](./ci-sim.md)

## What the AMIs are for
The SimKube AMIs are designed for:

- running SimKube simulations on EC2
- providing a consistent environment across runs
- reducing setup and dependency management
- running larger, longer simulations than you can run locally with SimKube
- running SimKube in CI pipelines

## Available AMIs
- SimKube AMI
Suitable for running on demand SimKube workloads and long running simulations.
- SimKube GitHub Runner AMI
Designed specifically with CI in mind. Use this AMI as an ephemeral SimKube GitHub Action Runner.

## What's included in the AMIs

| Feature                                             | SimKube AMI | SimKube GitHub Runner AMI |
|-----------------------------------------------------|:-----------:|:-------------------------:|
| Ubuntu 24.04 LTS Operating System                  | ✅          | ✅                        |
| A running Kubernetes cluster, with management tools| ✅          | ✅                        |
| All SimKube components*                            | ✅          | ✅                        |
| All SimKube dependencies                           | ✅          | ✅                        |
| Container runtime + system dependencies            | ✅          | ✅                        |
| GitHub Actions Runner software                     | ❌          | ✅                        |


Our AMIs are optimized for running simulations, and are not recommended for any other use cases.

> [!NOTE] more on SimKube components [here](../components/sk-ctrl.md)

## Next steps
- [Launch and use SimKube AMIs](./usage.md)
- [Configure GitHub Actions to run simulations on self-hosted runners](github-runners.md)
