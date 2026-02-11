<!--
template: docs.html
-->
# SimKube GitHub Action Runner AMI

[Applied Computing Research Labs](https://appliedcomputing.io) provides support for running simulations on self-hosted GitHub Actions runners that are backed by SimKube AMIs.

These runners are intended for teams that want reliable, repeatable simulation as part of their CI pipelines.

## When to use a SimKube GitHub Actions Runner
The primary use case for using the SimKube GitHub Actions Runner AMI is to run simulations in CI.

An example configuration using the SimKube GitHub Actions Runner AMI is available in the SimKube repo using the [simkube-ci-action](https://github.com/acrlabs/simkube-ci-action) GitHub action maintained by ACRL.

## Runner lifecycle
SimKube GitHub runners are self-hosted and managed by your organization.

- runners must be registered with GitHub at the repository or organization level
- authentication and registration follow GitHub's standard self-hosted runner process
- runners are currently ephemeral-only and designed to be launched via GitHub Actions

For more information on configuration self-hosted GitHub runners, please see the [instructions provided by GitHub](https://docs.github.com/en/actions/how-tos/manage-runners/self-hosted-runners/add-runners).

## Using the runners in workflows
Once registered, the runner can be targeted using [GitHub `runs-on` labels](https://docs.github.com/en/actions/how-tos/manage-runners/self-hosted-runners/apply-labels).

Example using our default labels:

```yaml
runs-on: [self-hosted, simkube, ephemeral]
```

## SimKube custom GitHub Actions
We have a set of custom GitHub actions for running SimKube in CI.

- for more visit the [simkube-ci-actions](https://github.com/acrlabs/simkube-ci-action) repo
- or see an example in our [Run SimKube in CI](./ci-sim.md) quick start guide
