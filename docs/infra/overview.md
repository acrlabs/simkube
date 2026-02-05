<!--
template: docs.html
-->
# Infrastructure Overview
SimKube can run wherever you run k8s, from local testing environments to automated CI pipelines. For running simulations at scale or integrating SimKube into CI workflows, we provide prebuilt infrastructure components to simplify setup and improve reliability.

This section documents those components and where you can use them.

## When to use these components
You may way to use the infrastructure components described here if you want:

- to run SimKube simulations in CI (for example, GitHub Actions)
- a repeatable, preconfigured SimKube environment
- to avoid maintaining your own base images or runners
- to run SimKube simulations in AWS

## What's included in this section
- Amazon Machine Images (AMIs) - Prebuilt EC2 images with SimKube and its dependencies installed and configured
- GitHub Actions Runners - Self-hosted runners built on top of our AMIs, designed for running SimKube workloads in CI.*

*Note: these runners are self-hosted by you in your AWS account using ACRL's runner AMI

The next steps cover how to use these components.

## What the following sections won't cover
- General AWS or EC2 concepts
- GitHub Actions Basics
- Building or customizing the AMIs themselves

However, we link to sone relevant outside resources.

## Next steps
- [Learn about SimKube AMI options](./amis.md)
- [Launch and use SimKube AMIs](./usage.md)
- [Configure GitHub Actions to run simulations on self-hosted runners](github-runners.md)

## Quick Start Guides
- [Run SimKube in AWS EC2](./run-sim.md)
- [Run SimKube in CI](./ci-sim.md)
