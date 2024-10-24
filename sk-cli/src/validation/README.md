# SimKube Trace Validation Checks

| code | name | description |
|---|---|---|
| W0000 | status_field_populated |  Indicates that the status field of a Kubernetes object in the trace is non-empty; status fields are updated by their controlling objects and shouldn't be applied "by hand".  This is probably "fine" but it would be better to clean them up (and also they take up a lot of space.  |

This file is auto-generated; to rebuild, run `make validation_rules`.
