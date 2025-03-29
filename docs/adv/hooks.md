<!--
template: docs.html
-->

# Simulation hooks

SimKube supports running arbitrary setup or cleanup scripts at a number of different points during the simulation
process.  The general method for configuring hooks is the same at each extension point: simply inject the following
command into the Simulation custom resource:

```yaml
- cmd: echo           # required
  args: ["foo"]       # required
  ignoreFailure: true # optional, will not abort the simulation on failure
  sendSim: true       # optional, will send the Simulation resource to the hook as JSON over stdin
```

## Extension points

There are four places where hooks can be injected:

### preStartHooks

Pre-start hooks run once before any other simulation setup; you can use these hooks to create additional namespaces, set
up monitoring, etc.

### postStopHooks

Similarly, post-stop hooks run once after _all_ simulation iterations have completed and after all other cleanup tasks
are complete.  They can be used to clean up any resources or do additional reporting on the simulation results
(extracting logs from relevant pods, for example).

### preRunHooks

Pre-run hooks run before _every_ iteration of the simulation, and can be used to re-create resources that should be
"fresh" at the beginning of each iteration.  They are the first thing the SimKube driver runs, before executing any
other setup.

### postRunHooks

Lastly, post-run hooks run at the end of _every_ simulation iteration, and can be used to clean up resources that might
pollute future simulation iterations.  They are the last thing the SimKube driver runs.

## Injecting hooks

If you are using `skctl` to run your simulation, you can provide a set of hooks via a YAML file similar to the
following, using the `--hooks` CLI argument:

```bash exec="on" result="yaml"
cat simkube/examples/hooks/example.yml
```

Note that you can specify multiple hooks files, separated by a colon, as in
`config/hooks/default.yml:config/hooks/autoscaler.yml`.  This will merge the four different types of hooks specified in
each hook file _in order_; in other words, hooks from earlier files in the list will run before hooks from later files
in the list.

Otherwise, you can specify the hooks as part of the Simulation custom resource object.

## Running hooks

All executables needed to run hooks must be present and on the path in the `sk-ctrl` pod (for pre-start and post-stop
hooks) or in the `sk-driver` pod (for pre-run and post-run hooks).  The standard Docker images built for SimKube include
`kubectl`, `curl`, and `jq` for this purpose.
