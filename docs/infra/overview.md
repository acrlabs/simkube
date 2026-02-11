<!--
template: docs.html
-->
# Prebuilt Simulation Environments Overview
SimKube can run wherever you run k8s, from local testing environments to automated CI pipelines. For running simulations at scale or integrating SimKube into CI workflows, [Applied Computing Research Labs](https://appliedcomputing.io) provides prebuilt infrastructure components to simplify setup and improve reliability.

This section documents those components and where you can use them.

## When to use these components
You may wish to use the infrastructure components described here if you want:

- to run SimKube simulations in CI (for example, GitHub Actions)
- a repeatable, preconfigured SimKube environment
- to avoid maintaining your own base images or runners
- to run SimKube simulations in AWS

## What's included
- Amazon Machine Images (AMIs) - Prebuilt EC2 images with SimKube and its dependencies installed and configured; available for free
- GitHub Actions Runners - Self-hosted runners built on top of our AMIs, designed for running SimKube workloads in CI; available for a small fee

> [!NOTE]
> These runners are self-hosted in your AWS account using ACRL's runner AMI.

The next steps cover how to use these components.

## Next steps
- [Learn about SimKube AMI options](./amis.md)
- [Launch and use SimKube AMIs](./usage.md)
- [Configure GitHub Actions to run simulations on self-hosted runners](github-runners.md)

## Quick Start guides
- [Run SimKube in AWS EC2](./run-sim.md)
- [Run SimKube in CI](./ci-sim.md)

## How to get support
- open an issue in the [SimKube GitHub repo](https://github.com/acrlabs/simkube/issues)
- message us in the [SimKube Slack Channel](https://kubernetes.slack.com/#simkube)
