<!--
project: SimKube
template: docs.html
-->

# Frequently Asked Questions

## How do you inspect or modify the contents of a trace file?

You can use the [msgpack-tools](https://github.com/ludocode/msgpack-tools) utility to view the contents of a trace file:

```
> msgpack2json -di path/to/trace
[
        "trackedObjects": {
            "apps/v1.Deployment": {
                "podSpecTemplatePath": "/spec/template"
            }
        }
    },
    [
        {
            "ts": 1711247936,
            "applied_objs": [],
            "deleted_objs": []
        },
        {
            "ts": 1711247936,
            "applied_objs": [
                {
                    "apiVersion": "apps/v1",
                    "kind": "Deployment",
                    "metadata": {
                        "annotations": {
                            "meta.helm.sh/release-name": "dsb-social-network",
                            "meta.helm.sh/release-namespace": "dsb"
                        },
                        "labels": {
                            "app.kubernetes.io/managed-by": "Helm",
                            "service": "compose-post-service"
                        },
                        "name": "compose-post-service",
                        "namespace": "dsb"
                    },
    ...
```
