<!--
project: SimKube
template: docs.html
-->

# skctl

`skctl` is the CLI for interacting with SimKube.  It's not required to use but it will make your life a lot easier.

## skctl export

```
export trace data

Usage:
  skctl export [flags]

Flags:
      --end-time string                   end time; can be a relative or absolute (local) timestamp
                                           (default "now")
      --excluded-labels stringArray       label selectors to exclude from the trace (key=value pairs)
      --excluded-namespaces stringArray   namespaces to exclude from the trace
                                           (default [kube-system,monitoring,local-path-storage,simkube,cert-manager,volcano-system])
  -h, --help                              help for export
  -o, --output string                     location to save exported trace
                                           (default "file:///tmp/kind-node-data")
      --start-time string                 start time; can be a relative duration or absolute (local) timestamp
                                              in ISO-8601 extended format (YYYY-MM-DDThh:mm:ss).
                                              durations are computed relative to the specified end time,
                                              _not_ the current time
                                           (default "-30m")
      --tracer-addr string                tracer server address
                                           (default "http://localhost:7777")

Global Flags:
  -v, --verbosity int   log level output (higher is more verbose) (default 2)
```

Export a trace from a running `sk-tracer` pod between the specified `--start-time` and `--end-time`, as well as
according to the specified filters.  The resulting trace will be stored in the `--output` directory.

## skctl run

```
run a simulation

Usage:
  skctl run [flags]

Flags:
  -h, --help              help for run
      --sim-name string   the name of simulation to run

Global Flags:
  -v, --verbosity int   log level output (higher is more verbose) (default 2)
```

## skctl rm

```
run a simulation

Usage:
  skctl rm [flags]

Flags:
  -h, --help              help for rm
      --sim-name string   the name of simulation to run

Global Flags:
  -v, --verbosity int   log level output (higher is more verbose) (default 2)
```
