<!--
template: docs.html
-->

# skctl

`skctl` is the CLI for interacting with SimKube.  It's not required to use but it will make your life a lot easier.

```bash exec="on" result="plain"
skctl --help
```

## skctl crd

```bash exec="on" result="plain"
skctl crd --help
```

Generate all of the necessary CustomResourceDefinitions for SimKube.

## skctl delete

```bash exec="on" result="plain"
skctl delete --help
```

## skctl export

```bash exec="on" result="plain"
skctl export --help
```

Export a trace from a running `sk-tracer` pod between the specified `--start-time` and `--end-time`, as well as
according to the specified filters.  The resulting trace will be stored in the `--output` directory.  Timestamps
can either be relative ("-2h", "now", etc) or absolute ("2024-01-01T12:00:00").  If you find a timestamp format
that isn't accepted or is parsed incorrectly, please [file an issue](https://github.com/acrlabs/simkube/issues/new?template=bug_report.md&title=incorrect%20timestamp%20parsing&labels=cli,bug).

## skctl run

```bash exec="on" result="plain"
skctl run --help
```

## skctl snapshot

```bash exec="on" result="plain"
skctl snapshot --help
```

Create a point-in-time snapshot of the configured objects that are currently running on a Kubernetes cluster.  Note
that, unlike `skctl export`, the snapshot command _does not require `sk-tracer` to be running on the cluster!_  This
means that you can pick an arbitrary starting point, create a trace file from there, and "let the simulation run to see
what happens".  The snapshot command will try to read your local Kubernetes credentials from, e.g., `~/.kube/config`,
and you will need read access to all namespaces on the cluster you're trying to snapshot.

The config file format is the same as for [sk-tracer](sk-tracer.md); there is an example in the [examples
folder](https://github.com/acrlabs/simkube/blob/main/examples/tracer_config.yml).

## skctl validate

```bash exec="on" result="plain"
skctl validate check --help
```

Validate a specified tracefile, looking for common errors or issues that may make your simulation less successful, or
fail outright.  You can use the `--fix` option to automatically fix errors as they come up, or use `skctl validate
explain` to understand a particular error better.

## skctl xray

```bash exec="on" result="plain"
skctl xray --help
```

Bring up a TUI for exploring a trace file; use `h/j/k/l` or arrow keys to navigate; spacebar to expand an item; escape
to collapse an item, and `q` to quit.
