<!--
template: docs.html
-->
# Infrastructure Overview

SimKube can run wherever you run k8s, from local testing environments to automated CI pipelines. For running simulations at scale or integrating SimKube into CI workflows, we provide prebuilt infrastructure components to simplify setup and improve reliability.

This section documents those components and when you should use them.

## When to use these components

You may way to use the infrastructure components described here if you want:

- to run SimKube simulations in CI (for example, GitHub Actions)
- a repeatable, preconfigured SimKube environment
- to avoid maintaining your own base images or runners
- to run SimKube simulations in AWS

If you are only running simulations locally, you likely don't need to read this section just yet.

## What's included in this section

- Amazon Machine Images (AMIs) - Prebuilt EC2 images with SimKube and its dependencies installed and configured
- GitHub Actions Runners - Self-hosted built on top of our AMIs, designed for running SimKube workloads in CI.*

*Note: these runners are self-hosted by you in your AWS account.

The next steps cover how to use these components.

## What the following sections won't cover

- General AWS or EC2 concepts
- GitHub Actions Basics
- Building or customizing the AMIs themselves

However, we will link to relevant outside resources where they are helpful.

## Next steps

- [Learn about SimKube AMI options](./amis.md)
- [Launch and use SimKube AMIs](./usage.md)
- [Configure GitHub Actions to run simulations on self-hosted runners](github-runners.md)
