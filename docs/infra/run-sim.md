<!--
template: docs.html
-->
# Run SimKube in AWS EC2
This guide is intended for users who want to run SimKube in EC2 for one off simulations or longer-lived simulation environments.

## Assumptions
- you have collected a trace from the cluster you want to simulate, if you still need to do this see [the sk-tracer docs](../components/sk-tracer.md).

## 0. AWS IAM Requirements
These are the basic AWS IAM permissions required to continue
```json
  "Effect": "Allow",
  "Action": [
    "ec2:DescribeImages",
    "ec2:DescribeInstances",
    "ec2:RunInstances",
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

## 1.Locate the SimKube AMI

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

## 2. Launch an EC2 instance from the AMI
- we recommend using the latest available SimKube AMI
- choose an instance type appropriate for your workload
- attach a keypair for ssh access

## 3. Connect to the instance
```sh
ssh ubuntu@<instance-public-ip>
```

## 4. Load your trace
Note: if your trace is in S3 you can skip this step, S3 is recommended for large trace files

Copy your trace to the instance, the default SimKube trace location is /data/trace:

```sh
scp your_trace_file ubuntu@<instance-ip>:/data/trace
```

## 5. Run your simulation
```sh
skctl run my-simulation --trace-path s3://your-simkube-bucket/path/to/trace
```

## 6. Evaluate your results
[COMING SOON!]
