<!--
template: docs.html
-->

# Traces

The SimKube tracer collects timeseries data about the events happening in a live Kubernetes cluster and exports that
data to a trace file for future replay and analysis.  These trace files can then be stored in a cloud provider or
downloaded locally.  We describe configuration options for each of these use cases.

## Cloud storage

We support exporting traces to Amazon S3, Google Cloud Storage, and Microsoft Azure Storage through the
[object\_store](https://docs.rs/object_store/latest/object_store/) crate.  The `sk-tracer` and `sk-driver` pods need to
be configured with the correct permissions to write and read data to your chosen cloud storage.  One option is to inject
environment variables into the pod that object\_store understands.

- Amazon S3: use the `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` environment variables
- Google Cloud Storage: use the `GOOGLE_SERVICE_ACCOUNT` environment variable and inject your service account JSON file
  into the `sk-tracer` pod
- Microsoft Azure: use the `AZURE_STORAGE_ACCOUNT_NAME` and `AZURE_STORAGE_ACCOUNT_KEY` environment variables

The object\_store crate will try other authentication/authorization methods if these environment variables are not set
(for example, it will try to get credentials from the instance metadata endpoint for AWS), so these are not the only
ways to grant permissions to the tracer and the driver.  Configuring these permissions is beyond the scope of this
documentation, and we encourage you to consult the IAM documentation for your chosen cloud provider(s).

## Local storage

If you do not have access to (or do not want to use) cloud storage, you can also save a trace file to local storage
using, for example, `skctl export -o file:///path/to/trace`.  However, using this trace file in the simulator is a bit
more complicated; it will need to be injected into the node(s) where your Simulation driver pods will run, and then
volume-mounted into the driver pod.  If you are running locally via `kind`, you can add the following block to your
`kind` config to mount the trace file directory on your laptop into the kind nodes:

```yaml
  - role: worker
    extraMounts:
      - hostPath: /tmp/kind-node-data
        containerPath: /data
```

From there, when you run a simulation, you need to specify the trace data using `skctl run --trace-path
file:///data/trace`.  This location is the location _inside the Kind node docker container_, not inside the driver pod.
SimKube will automatically construct the appropriate volume mounts so that the driver pod can reference the trace.
