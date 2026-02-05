<!--
template: docs.html
-->
# SimKube GitHub Action Runner AMI

Simkube provides support for running simulations on self-hosted GitHub Actions runners that are backed by the SimKube AMIs.

These runners are intended for teams that want reliable, repeatable simulation as part of their CI pipelines.

## When to use a SimKube GitHub Actions Runner
The primary use case for using the SimKube GitHub Actions Runner AMI is to run simulations in CI.

For an example, we use the SimKube GitHub Actions Runner AMI for end to end testing the SimKube core repo using our [simkube-ci-action](https://github.com/acrlabs/simkube-ci-action).

## Runner lifecycle
SimKube runners are self-hosted and managed by you.

- runners must be registered with GitHub at the repository or organization level
- authentication and registration follow GitHub's standard self-hosted runner process
- runners are currently ephemeral only and designed to be launched via GitHub Actions

If you have not previously setup GitHub hosted runners GitHub provides a set of instructions [here](https://docs.github.com/en/actions/how-tos/manage-runners/self-hosted-runners/add-runners).

## Using the runners in workflows
Once registered, the runner can be targeted in workflows using labels

Example using our default labels:
```yaml
runs-on: [self-hosted, simkube, ephemeral]
```

## SimKube custom GitHub Actions
We have a set of custom GitHub actions for running SimKube in CI.
- for more visit the [simkube-ci-actions](https://github.com/acrlabs/simkube-ci-action) repo
- or see an example in our [Run SimKube in CI](./ci-sim.md) quick start guide

## Security
- runner execute the workflow code with the permissions of the runner host
- secrets are provided by GitHub at runtime and masked
- access to the AWS resources are controlled by instance's IAM role

## Updates and maintenance
- runner updates are delivered via new AMIs
- existing runners are not updated automatically
- we recommend periodically redeploying runner to stay up to date
