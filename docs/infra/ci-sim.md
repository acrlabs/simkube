<!--
template: docs.html
-->
# Run SimKube in CI

This quickstart guide explains how to use SimKube in CI using GitHub Actions and AWS EC2.

## 0. AWS IAM Requirements

These are the basic AWS IAM permissions required to continue
```json
  "Effect": "Allow",
  "Action": [
    "ec2:DescribeImages",
    "ec2:DescribeInstances",
    "ec2:RunInstances",
    "ec2:CreateTags",
  ],
  "Resource": "*"
```

Note: if using SSM you may need additional permissions to launch instances or use SSM

- If you plan to import or export traces in AWS S3 you will need permissions for those resources.
```json
{
  "Effect": "Allow",
  "Action": [
    "s3:PutObject",
    "s3:GetObject"
  ],
  "Resource": "arn:aws:s3:::<bucket-name>/*"
}
```

You will need to generate a `key pair` in AWS for the IAM user you are using to access AWS resources. Hang onto those; you will need them when you configure the secrets.

AWS provides instructions on creating key pairs in AWS IAM via the console or CLI [here](https://docs.aws.amazon.com/IAM/latest/UserGuide/access-keys-admin-managed.html#admin-create-access-key).

## 1. GitHub Permissions

To use SimKube in CI the GitHub account will need:
- permissions to access code and manage custom runners
- a method of accessing those permissions

### Example using a fine grained PAT:

#### Setup the PAT in GitHub:
- Go to user `Settings`
- Click `Developer settings`
- Under `Personal access tokens`
- Choose `Fine-grained tokens`
- Select the `Resource owner`: if the repo is not owned by you it will send an access request to the owner(s) of the repos you select
- Give the token a descriptive `Token name` and `Description`
- The `Request message` should give some context to the admin
- Choose an `Expiration` that meets your organization's policy requirements
- Select `Only select repositories`
- Choose the repositories you want to run SimKube in
- Click `Add permissions`
- Select Read and Write access for `Actions` and `Administration`
  Note: `metadata` will be selected by default
- Click `Generate token and request access`
- In the next step we will add the PAT to our secrets

## 2. Configure secrets
Add the following secrets to the repo you will be testing in
- `SIMKUBE_RUNNER_PAT` - PAT with repo scope created in Step 1
- `AWS_ACCESS_KEY_ID` - AWS access key created in Step 0
- `AWS_SECRET_ACCESS_KEY` - AWS secret key created in Step 0

## 3. Create a GitHub Actions workflow
We will be using a custom action created by ACRL called [simkube-ci-action](https://github.com/acrlabs/simkube-ci-action). Our custom action simplifies the setup and teardown of ephemeral runners so you can focus on running impactful simulations in CI.
To use `simkube-ci-action` use the `launch-runner` and `run-simulation` custom actions in your workflow.

### A basic action workflow file might look like:

```yaml
---
name: Run simulation
on:
  workflow_dispatch:
  push:
    branches:
      - "main"
jobs:
  launch-runner:
    runs-on: ubuntu-latest
    steps:
      - name: Setup SimKube GitHub Action runner
        uses: acrlabs/simkube-ci-action/actions/launch-runner@main
        with:
          instance-type: m6a.large
          aws-region: us-west-2
          subnet-id: subnet-xxxx
          security-group-ids: sg-xxxx
          simkube-runner-pat: ${{ secrets.SIMKUBE_RUNNER_PAT }}
          aws-access-key-id: ${{ secrets.AWS_ACCESS_KEY_ID }}
          aws-secret-access-key: ${{ secrets.AWS_SECRET_ACCESS_KEY }}
  run-simulation:
    needs: launch-runner
    runs-on: [self-hosted, simkube, ephemeral]
    steps:
      - uses: actions/checkout@v5
      - name: Run simulation
        uses: acrlabs/simkube-ci-action/actions/run-simulation@main
        with:
          simulation-name: your-sim-name
          trace-path: path/to/your/trace
```

## 4. Test your SimKube workflow
Test your workflow by manually dispatching it in the actions menu or pushing some code

Currently `simkube-ci-action` pass/fail. The simulation either runs to completion or it fails. We do not currently have a method for injecting evaluation criteria for simulations.

A successful simulation will exit with code 0 and you will see a `✓ Simulation completed successfully!` in the actions logs.

A failed simulation will exit with a non-zero exit code failing the CI action and printing a detailed failure summary.

## 5. Evaluating your results
Prometheus and Grafana are installed natively. Users can view simulation results by connecting to the Grafana pod on your EC2 instance.

See [Evaluate your results](./evaluate.md).

> [!NOTE]
> `simkube-ci-action` runners are epehmeral-only and all data from the simulation is lost.
> In the future we expect to expose functionality that will allow data to be sent to external prometheus endpoints.
