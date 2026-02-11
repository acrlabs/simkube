<!--
template: docs.html
-->
# Using SimKube AMIs
How to locate, run and use SimKube AMIs.

## Locating the AMI
SimKube AMIs are published to the AWS Marketplace and versioned.

You can find the latest AMI by:
- searching for AMIs owned by ACRL
- using the AWS CLI to filter by name and region

```sh
aws ec2 describe-images \
  --owners 174155008850 \
  --filters "Name=name,Values=simkube-*" \
  --query "Images[].{
    ImageId: ImageId,
    Name: Name,
    CreationDate: CreationDate
  }" \
  --region us-west-2 \
  --output table
```

## Launching the AMI
You can launch an EC2 instance using the SimKube AMI via:
- the AWS console
- the AWS CLI
- infrastructure as code (IAC) tools like Terraform / Pulumi

Here are the AWS docs on [launching EC2 instances](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/LaunchingAndUsingInstances.html).

When launching an instance:

- select an appropriate instance type for your use
- configure your network and security groups
- provide an SSH key pair

## Instance sizing
SimKube simulations are a compute bound workload.

Our recommended default instance type is `c7a.2xlarge`:
- 8 vCPUs
- 16 GiB RAM
- Strong price/performance for compute intensive workloads
- Cost efficient AMD architecture

This instance size supports most simulations reliably but we encourage you to experiment a little to find the right instance size for your specific simulation needs.

[!NOTE] Avoid burstable instanc types like `t3`, `t4g` as they are not well suited to sustained simulations.

## Accessing the instance
Instances launched from the SimKube AMI support SSH access.

- use the default `ubuntu` user
- authenticate using the SSH key pair specified at launch

## AMI versioning and updates
Each AMI is versioned and immutable.

- any updates will be delivered by publishing a new AMI
- existing instances are not modified
- we recommend frequently updating for the latest bug fixes, improvements and base OS security updates

## Limitations
- The AMIs are designed for and only available in AWS EC2
- long-running simulation environments should be managed and monitored by the user

## Next steps
- [Configure GitHub Actions to run simulations on self-hosted runners](github-runners.md)


[TODO] add appropriate instance type
