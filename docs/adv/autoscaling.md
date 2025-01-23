<!--
template: docs.html
-->

# Autoscaling with SimKube

When running your simulations, you probably don't want to have to manually create a bunch of KWOK nodes every time.
Fortunately, both [Cluster Autoscaler](https://github.com/kubernetes/autoscaler) and [Karpenter](https://karpenter.sh)
(the two most popular cluster autoscalers for Kubernetes) support KWOK, which means you can have them autoscaler your
simulated cluster so you don't have to manually create virtual nodes.

## Cluster Autoscaler instructions

You will need to run Cluster Autoscaler with the `--cloud-provider kwok` argument.  The KWOK Cluster Autoscaler provider
expects two ConfigMaps to be present; the first tells the KWOK cloudprovider what to use for its Cluster Autoscaler
NodeGroups:

```yaml
# provider-config.yml
apiVersion: v1
kind: ConfigMap
metadata:
  name: kwok-provider-config
  namespace: kube-system
data:
  config: |
    ---
    apiVersion: v1alpha1
    readNodesFrom: configmap
    nodegroups:
      fromNodeLabelKey: "kwok-nodegroup"
    configmap:
      name: kwok-provider-templates
```

The second enumerates the node types that the KWOK cloudprovider supports:

```yaml
# provider-templates.yml
apiVersion: v1
kind: ConfigMap
metadata:
  name: kwok-provider-templates
  namespace: kube-system
data:
  templates: |
    ---
    apiVersion: v1
    kind: List
    items:
      - apiVersion: v1
        kind: Node
        metadata:
          annotations:
            kwok.x-k8s.io/node: fake
            kowk-nodegroup: node-group-1
          labels:
            node.kubernetes.io/instance-type: c5d.9xlarge
            topology.kubernetes.io/zone: us-west-1a
            type: virtual
        status:
          allocatable:
            cpu: 31
            ephemeral-storage: 900Gi
            memory: 71Gi
            pods: 110
          capacity:
            cpu: 36
            ephemeral-storage: 900Gi
            memory: 72Gi
            pods: 110
```

The KWOK cloudprovider will automatically apply a `kwok-provider: true` taint to the nodes it generates with a
`NoSchedule` effect on them.  SimKube will likewise apply the corresponding toleration to the virtual pods it creates.

For more information on running and configuring KWOK for Cluster Autoscaler, see the
[README](https://github.com/kubernetes/autoscaler/tree/master/cluster-autoscaler/cloudprovider/kwok).

## Karpenter instructions

The core [karpenter repo](https://github.com/kubernetes-sigs/karpenter) includes a KWOK provider for karpenter.  There
are some initial instructions in there for installing the karpenter+KWOK binary into your cluster.  Once it's installed,
it will automatically use KWOK to scale up nodes in the cluster just like Cluster Autoscaler.  As with Cluster
Autoscaler, KWOK applies the `kwok-provider=true:NoSchedule` taint to the nodes it creates.

Unlike Cluster Autoscaler, karpenter does not take in a list of Kubernetes Node specs to determine what instances it
launches.  Instead, it uses a hard-coded list of "generic" instance types which roughly map to standard instance
offerings by the major cloud providers.  If you want to run Karpenter with a different set of configured instances, you
need to modify the [embedded `instance_types.json`](https://github.com/kubernetes-sigs/karpenter/blob/main/kwok/cloudprovider/instance_types.json)
file and rebuild Karpenter.
