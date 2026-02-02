<!--
template: docs.html
-->
# Using SimKube AMIs

How to locate, run and use SimKube AMIs.

## Locating the AMI

SimKube AMIs are published to the AWS Marketplace and versioned.

You can find the latest AMI by:
- searching for AMIs owned by the ACRL AWS account
- using the AWS CLI to filter by name and region

```sh
aws ec2 describe-images \
  --owners 174155008850 \
  --filters "Name=name,Values=simkube-x86-64-*" \
  --query "Images[].{
    ImageId: ImageId,
    Name: Name,
    CreationDate: CreationDate,
    SimKubeVersion: SimKubeVersion
  }" \
  --region us-west-2 \
  --output table
```

## Launching the AMI

You can launch an EC2 instance using the SImKube AMI via:
- the AWS console
- the AWS CLI
- infrastructure as code (IAC) tools like Terraform / Pulumi

Here is the AWS docs on [launching EC2 instances](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/LaunchingAndUsingInstances.html).

When launching an instance:

- select an appropriate instance type for your use
- configure your network and security groups
- provide an SSH key pair if access is desired

## Accessing the instance

Instances launched from the SimKube AMI support SSH access.

- use the default `ubuntu` user
- authenticate using he SSH key pair specified at launch**
- if no keypair is provided at launch and you need access use ec2-connect to push one if it is enabled in your account

Link to the AWS docs on [connecting to your EC2 instance](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/connect.html).

## AMI versioning and updates

Each AMI is versioned and immutable.

- any updates will be delivered by publishing a new AMI
- existing instances are not modified
- we recommend frequently updating to the latest bug fixes, improvements and base OS security updates

## Limitations

- The AMIs are designed for and only available in AWS EC2
- long-running simulation environments should be managed and monitored by the user

## Next steps
- [Configure GitHub Actions to run simulations on self-hosted runners](github-runners.md)
