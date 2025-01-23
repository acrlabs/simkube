<!--
template: docs.html
-->

# Running your first simulation

Once you've gone through the steps to install SimKube in your environment, you're ready to run your first simulation!

## Step 1: Collect a trace

Use the `skctl` CLI tool to collect a trace from your production cluster (where you're running `sk-tracer`):

```
> kubectl port-forward -n simkube pod/sk-tracer-depl-6d559b799-ln8gk 7777:7777
> skctl export -o s3://your-simkube-bucket/path/to/trace
```

Alternately, if you don't have `sk-tracer` running anywhere, you can generate a "point-in-time" snapshot of your
production cluster with the following command:

```
> skctl snapshot -c config.yml
```

The config file referenced should be in the same format as expected by `sk-tracer`.  Here's a basic one you can use:

```yaml
# config.yml
trackedObjects:
  apps/v1.Deployment:
    podSpecTemplatePath: /spec/template
```

## Step 2: Create some virtual nodes

SimKube is going to create a bunch of fake pods during the simulation run, and it will need someplace to schedule them.
We're going to create a virtual node, managed by KWOK, for this.

```yaml
# node.yml
apiVersion: v1
kind: Node
metadata:
  annotations:
    kwok.x-k8s.io/node: fake
  labels:
    node.kubernetes.io/instance-type: c5d.9xlarge
    topology.kubernetes.io/zone: us-west-1a
    type: virtual
  name: fake-node-1
spec:
  taints:
  - effect: NoSchedule
    key: kwok-provider
    value: "true"
status:
  allocatable:
    cpu: 35
    ephemeral-storage: 900Gi
    memory: 71Gi
    pods: 110
  capacity:
    cpu: 36
    ephemeral-storage: 900Gi
    memory: 72Gi
    pods: 110
```

```
> kubectl apply -f node.yml
```

Now, you should see your node appear in the list of cluster nodes, posting a "Ready" status:

```
> kubectl get nodes
NAME                    STATUS   ROLES           AGE   VERSION
fake-node-1             Ready    <none>          82s   kwok-v0.5.1
simkube-control-plane   Ready    control-plane   12m   v1.27.3
simkube-worker          Ready    <none>          12m   v1.27.3
```

## Step 3: Run your simulation!

```
> skctl run my-first-simulation --trace-path s3://your-simkube-bucket/path/to/trace --duration +5m
running simulation my-first-simulation
```

You should see that the simulation object has been created:

```
> kubectl get simulation my-first-simulation
NAME      START TIME   END TIME   STATE
testing                           Initializing
```

During the "Initializing" phase, `sk-ctrl` is setting up a temporary high-resolution Prometheus pod to scrape data from
the simulation, as well as configuring other needed components.  After 30-50 seconds, you should see the simulation
transition to "Running":

```
> kubectl get simulation my-first-simulation
NAME      START TIME             END TIME   STATE
testing   2024-03-01T04:58:48Z              Running
```

Once your simulation is over, it should move into the "Finished" state:

```
> kubectl get simulation my-first-simulation
NAME      START TIME             END TIME               STATE
testing   2024-03-01T04:58:48Z   2024-03-01T04:59:06Z   Finished
```

Congratulations!  You did it!  Read on to learn more about how you can do some more advanced things in your simulated
environment.
