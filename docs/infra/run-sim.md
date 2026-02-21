<!--
template: docs.html
-->
# Run SimKube in AWS EC2

This guide is intended for users who want to run SimKube in EC2 for one off simulations or longer-lived simulation
environments.

## Assumptions

- you have collected a trace from the cluster you want to simulate, if you still need to do this see [the sk-tracer docs](../intro/running.md).
- you have sufficient permissions to managed the AWS resources described, for more on this see the AWS Permissions
  section on our [usage](./usage.md) page.

## 0. Locate the SimKube AMI

### Via the AWS CLI

```sh
aws ec2 describe-images \
  --owners 174155008850 \
  --filters "Name=name,Values=simkube-x86-64-*" \
  --query "Images[].{
    ImageId: ImageId,
    Name: Name,
    CreationDate: CreationDate
  }" \
  --region us-west-2 \
  --output table
```

### Via the AWS Console

  - Open the EC2 Console
  - Navigate to `AMIs`
  - Filter by:
    Owner: Owned by another account
    Owner ID: 174155008850
  - Search by name: `simkube-ami-*`

## 1. Launch an EC2 instance from the AMI

- we recommend using the latest available SimKube AMI
- choose an instance type appropriate for your workload
- attach a keypair for ssh access

## 2. Connect to the instance

```sh
ssh ubuntu@<instance-public-ip>
```

> [!NOTE]
> The default username to use to connect to your EC2 instance `ubuntu`, not `ec2-user`.

## 3. Load your trace
>
> [!NOTE]
> For simplicity and ease of use, we recommend using AWS S3 to store your trace files.  If your trace files are in S3,
> you can skip this step; SimKube will need additional IAM permissions to access your S3 bucket.

Copy your trace to the instance, the default SimKube trace location is `/var/kind/cluster/trace`:

```sh
scp your_trace_file ubuntu@<instance-ip>:/var/kind/cluster/trace
```

> [!WARNING]
> The trace file path on the EC2 host is not the same as the trace file path specified in the Simulation custom resource.
> This is because there's three layers of indirection for running on a local trace: the EC2 host gets mounted into the
> kind docker container which gets mounted into the SimKube pod.

## 4. Run your simulation

```sh
skctl run my-simulation --trace-path s3://your-simkube-bucket/path/to/trace
```

> [!NOTE]
> --trace-path defaults to file:///data/trace so this field is optional for local simulations

More information on running simulations with SimKube can be found [here](https://github.com/acrlabs/simkube/blob/main/docs/intro/running.md).

You can check the status of your simulation by running:

```sh
kubectl get simulation my-sim-name
```

> [!NOTE]
> Simulations will start in the `Initializing` state progress to `Running` once they have been scheduled.
> Finally, the simulation will complete with either a `Failed` or `Finished` state.

## 5. Evaluate your results

Prometheus and Grafana are installed natively. Users can view simulation results by connecting to the Grafana pod on
your EC2 instance.

See [Evaluate your results](./evaluate.md).
