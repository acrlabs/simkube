<!--
template: docs.html
-->
# Using SimKube AMIs

How to locate, run and use SimKube AMIs.

## AWS IAM Permissions

The basic AWS IAM permissions required to run SimKube AMIs.

```json
  "Effect": "Allow",
  "Action": [
    "ec2:DescribeImages",
    "ec2:DescribeInstances",
    "ec2:RunInstances",
  ],
  "Resource": "*"
```

> [!NOTE]
> SSM requires additional permissions, see:
> [Add SSM permissions to an IAM > role](https://docs.aws.amazon.com/systems-manager/latest/userguide/getting-started-add-permissions-to-existing-profile.html)
> and [Connect to EC2 via > SSM](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/connect-with-systems-manager-session-manager.html)

- If you plan to import or export traces in AWS S3 you will need permissions for those resources in S3.

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

We have included a [full IAM policy example](../ref/aws_iam_policy.md) in our reference section.

## Locating the AMIs

SimKube AMIs are published to the AWS Marketplace and versioned.

You can find the latest AMIs by:

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

This instance size supports most simulations reliably but we encourage you to experiment to find the right instance size
for your specific simulation needs.

> [!NOTE]
> Avoid burstable instance types like `t3`, `t4g` as they are not well suited to sustained simulations.

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
