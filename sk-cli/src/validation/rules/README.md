# SimKube Trace Validation Checks

| code | name | description |
|---|---|---|
| W0000 | status_field_populated | Indicates that the status field of a Kubernetes object in the trace is non-empty; status fields are updated by their controlling objects and shouldn't be applied "by hand".  This is probably "fine" but it would be better to clean them up (and also they take up a lot of space. |
| E0001 | service_account_missing | A pod needs a service account that is not present in the trace file.  The simulation will fail because pods cannot be created if their service account does not exist. |

This file is auto-generated; to rebuild, run `make validation_rules`.
