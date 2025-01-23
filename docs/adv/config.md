<!--
template: docs.html
-->

# SimKube Configuration

The following section describes some options for customizing the behaviour of your SimKube installation; if you are
using the provided [kustomize](https://github.com/acrlabs/simkube/tree/master/k8s/kustomize) manifests, you can update
or override these values there.

### Configuration of `sk-tracer`

The SimKube tracer runs in a real cluster and collects data about changes to objects in that cluster.  You can configure
what objects it watches via a config file, which is injected into the `sk-tracer` pod as a ConfigMap; if you are using
the provided kustomize manifests, you can override the `tracer-config.yml` data in the provided ConfigMap.  Here is an
example config that tells sk-tracer to watch Deployments, Jobs, and StatefulSets:

```yaml
trackedObjects:
  apps/v1.Deployment:
    podSpecTemplatePath: /spec/template
  batch/v1.Job:
    podSpecTemplatePath: /spec/template
  apps/v1.StatefulSet:
    podSpecTemplatePath: /spec/template
```

> [!NOTE]
> SimKube does some sanitization of the resources it watches, which is why it needs to know where the
> `podSpecTemplatePath` is; especially for custom resources, the path to the `podSpecTemplate` is not necessarily
> standard or well-known.  In a future version of SimKube we'll make this parameter optional for all "standard"
> Kubernetes objects.

`sk-tracer` needs an RBAC policy that grants "get", "list" and "watch" access to all configured objects in the cluster,
as well as pods.  For example, if you use the above configuration, you will need the following RBAC policy attached to
the service account used by `sk-tracer`:

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: sk-tracer
rules:
- apiGroups: [""]
  resources: ["pods"]
  verbs: ["get", "watch", "list"]
- apiGroups: ["apps/v1"]
  resources: ["deployment", "statefulset"]
  verbs: ["get", "watch", "list"]
- apiGroups: ["batch/v1"]
  resources: ["job"]
  verbs: ["get", "watch", "list"]
```

### Configuring `sk-ctrl`

The SimKube controller just needs the SimKube custom resources installed in the target environment, and needs no other
configuration.

The SimKube controller needs, at a minimum, write access for all of the objects that it will be simulating.  In theory,
since this is an isolated (or potentially even local) environment, it should be safe to give it `cluster-admin`, which
is probably the easiest way to configure it.
