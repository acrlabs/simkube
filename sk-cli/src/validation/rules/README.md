# SimKube Trace Validation Checks

| code | name | description |
|---|---|---|
| W0000 | status_field_populated | Indicates that the status field of a Kubernetes object in the trace is non-empty; status fields are updated by their controlling objects and shouldn't be applied "by hand".  This is probably "fine" but it would be better to clean them up (and also they take up a lot of space. |
| E0001 | service_account_missing | A Pod needs a ServiceAccount resource that is not present in the trace file.  The simulation will fail because pods cannot be created if the ServiceAccount does not exist. |
| E0002 | envvar_secret_missing | A Pod needs a Secret environment variable that is not present in the trace file.  The simulation will fail because pods cannot be created if the Secret does not exist. |
| E0003 | envvar_configmap_missing | A Pod needs a ConfigMap environment variable that is not present in the trace file.  The simulation will fail because pods cannot be created if the ConfigMap does not exist. |
| E0004 | volume_secret_missing | A Pod needs a Secret volume that is not present in the trace file.  The simulation will fail because pods cannot be created if the Secret does not exist. |
| E0005 | volume_configmap_missing | A Pod needs a ConfigMap volume that is not present in the trace file.  The simulation will fail because pods cannot be created if the ConfigMap does not exist. |

This file is auto-generated; to rebuild, run `make validation_rules`.
