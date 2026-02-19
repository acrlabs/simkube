<!--
template: docs.html
-->

# Transforming SimKube trace files with SKEL

The SimKube Expression Language is a text-based format that defines a list of transformations to be performed on a trace
file.  Each expression in a SKEL file is evaluated in order against each event in the trace file; the event list in the
trace file is traversed in sequence exactly once, and each event has the entire set of SKEL expressions evaluated
against it before moving on.  This limits the types of modifications you can perform against a trace: specifically,
loops and conditionals are explicitly not supported, nor can you have a SKEL modification that depends on the results of
an evaluation for an earlier or later trace event.  Despite these limitations, SKEL is a surprisingly expressive
language that can perform most common operations against a trace file.

# Format of a SKEL file

Each expression in a SKEL file is separated by a semi-colon (`;`).  White space, including new lines, does not matter
and is ignored.  Comments can be inserted anywhere and are prefixed by a hash sign (`#`).  All characters after a
comment marker are ignored until the end of the line.  Here is an example SKEL file:

```
# remove all status fields for every object in the trace:
remove(status);

# remove node tolerations for objects in the trace
# with a specific node selector:
remove(
    spec.template.spec.nodeSelector."simkube.dev/foo" == "bar",
    spec.template.spec.tolerations);

# remove all node selectors for objects in the trace after 10 minutes:
remove(@t >= 10m, spec.template.spec.nodeSelector);

# remove all container injected environment variables that
# depend on a secret key reference:
remove(
    $x: = spec.template.spec.containers[*].env[*]
        | exists($x.valueFrom.secretKeyRef),
    $x);
```

# SKEL expressions

As you can see from the above example, a SKEL expression has three parts: the command or operation name, an (optional)
selector field, and the target of the operation.  The full list of supported commands and their arguments is in the
[SKEL reference](../ref/skel.md).  The arguments to the commands are described in more detail below:

## Selectors

The first (optional) argument to SKEL commands is a "resource selector".  If no selector is provided, the operation is
performed against every event in the trace.  Users can also optionally use the star operator (`*`) to indicate "all
trace events".  In other words, these two expressions are equivalent:

```
remove(status);
remove(*, status);
```

Selectors can be chained together using the "and" operator (`&&`).  For example:

```
remove(sel1 && sel2, ...);
```

applies the `remove` operation to all events that match both `sel1` and `sel2`.

There are two types of selectors that allow more specific targeting of SKEL operations: timestamp selectors and resource
selectors.

### Timestamp selectors

A timestamp selector allows filtering based on "when" in the trace file an event occurs.  A timestamp
selector always starts with the "timestamp marker": `@t`, and any binary comparison operation is supported (`==`, `!=`,
`<=`, `>=`, `<`, `>`).  The right-hand-side of the comparison can either be a relative time, which must be suffixed by a
unit character (`s`, `m`, or `h` for seconds, minutes, or hours respectively), or an absolute time, which has no suffix.
For example, these are both valid timestamp selectors:

```
# every event after 10 minutes into the simulation
@t >= 10m

# every event after the absolute timestamp 12345 in the trace
@t >= 12345
```

The absolute timestamp variant is a Unix timestamp that is _compared to the original timestamps recorded in the trace
file_, and thus is slightly less convenient to use.  For example, if the trace file's first event occurred on December
1, 2025 at 12:00:00 (i.e., Unix timestamp `1764590400`), the trace selector of `@t >= 1764590300` will match every event
in the trace.

### Resource selectors

SKEL resource selectors allow filtering based on the type of object recorded in the trace.  Resource selectors use a
slightly-modified "path reference" to refer to fields in an object.  This syntax is similar to that supported by, e.g.,
`jq`, with a slight extension to support wildcards for array entries in an object.  A resource selector specifies a
resource field together with an operation; the resource field indicates "where" in the object's structure to look.
Both of these are valid resource field specifiers:

```
# reference the "simkube.dev/foo" object label
metadata.labels."simkube.dev/foo"

# reference all containers in the pod template spec
spec.template.spec.containers[*]
```

Valid characters for resource selectors are all alphanumeric characters, along with `-`, `_`, `.`, and `/`.  If the
selector field contains either `.` or `/` characters, that portion of the field _must_ be enclosed in double-quotes, as
shown in the label selector above.

Operations supported for resource selectors are (currently) "equals", "not equals", "exists", and "does not exist";
equals and not equals use the standard binary operators (`==` and `!=`), whereas the latter two operators use
function-like wrappers.  All of the following are valid resource selectors:

```
metadata.labels."simkube.dev/foo" == "bar"
metadata.labels."simkube.dev/foo" != "bar"
exists(metadata.labels."simkube.dev/foo")
!exists(metadata.labels."simkube.dev/foo")
```

> [!WARNING]
> There is a tricky subtlety for operations other than "exists" or "not exists": specifically, the field reference will
> only match where the path exists _AND_ the condition holds.  For example, in the following array:
>
>     [{"name": "container1"}, {"image": "container2"}, {"name": "container3"}]
>
> if resource selector is `name != "container1`, only the last element will match the selector and _not_ the middle
> array element, because the middle array element does not have a name field to test.
>
> This behaviour may change in a future version of SKEL.

### Variable resource selectors

A special case/type of resource selector is the _variable resource selector_.  Such selectors allow you to define a
variable reference that can be referred back to later on in the selector or in the target of the operation.  Variables
in SKEL always start with a dollar sign (`$`) and are defined using the variable assignment operator (`:=`).  Variable
selectors always reference a _set of matching paths in the object_ (and, to be explicit can refer to more than one
matching path when an array index wildcard is used).  The syntax for a variable resource selector is:

```
$x := sel | condition on $x
```

The `|` is read as "such that", so the entire expression should be interpreted as "define `$x` as the set of resource
paths pointed to by `sel` _such that_ the condition on `$x` holds".  For example, the following is a valid variable
resource selector:

```
$x := spec.template.spec.containers[*].env[*]
    | exists($x.valueFrom.secretKeyRef)
```

This defines `$x` to be the set of paths pointing to container environment variables that include a `secretKeyRef` in
their `valueFrom` field.  For example, if this is the resource's `podTemplateSpec` definition:

```json
"spec": {
  "template": {
    "spec": {
      "containers": [
        {
          "env": [
            {"valueFrom": {"secretKeyRef": "SOME_SECRET"}},
            {"FOO": "BAR"}
          ]
        },
        {"env": [{"ASDF": "QWERTY"}]},
        {
          "env": [
            {"FOO": "BAR"},
            {"valueFrom": {"secretKeyRef": "SOME_SECRET"}},
            {"valueFrom": {"secretKeyRef": "SOME_SECRET_2"}}
          ]
        }
      ]
    }
  }
}
```

then the above variable resource selector for `$x` would evaluate to

```
{
  spec.template.spec.containers[0].env[0],
  spec.template.spec.containers[2].env[1],
  spec.template.spec.containers[2].env[2],
}
```

Each resource selector can only define a _single_ variable; however, if selectors are chained with `&&`, each selector
can define a different variable, and the results of previous variables can be used in later selectors.

## Operation effects and targets

The second argument to a SKEL command is the "target" or the "effect" of the operation.  These targets reference fields
in the matched resource, and follow all the same rules for resource fields described above.  These targets can also
reference variables defined by the resource selectors, and will apply the effect to all paths contained within the
variable.  For example, the following expression specifies `$x` as the target of the remove operation, and will remove
all environment fields that include a `secretKeyRef`:

```
remove($x := spec.template.spec.containers[*].env[*]
    | exists($x.valueFrom.secretKeyRef), $x);
```

If this operation is applied to the pod template spec above, the result will be

```json
"spec": {
  "template": {
    "spec": {
      "containers": [
        {"env": [{"FOO": "BAR"}]},
        {"env": [{"ASDF": "QWERTY"}]},
        {"env": [{"FOO": "BAR"}]}
      ]
    }
  }
}
```

# Applying SKEL transformations

SKEL transformations can be applied by the SimKube CLI using the `transform` subcommand:

```
> skctl transform cronjob.sktrace sanitize.skel -o output.sktrace

Applying all transformations from sanitize.skel to cronjob.sktrace...
 ✅ ██████████████████████████████████████████████████████████ 2/2

All done!  Transformed trace written to output.sktrace.

Summary of changes:
----------------------------------------------------------------------
  Trace events matched: 46
  Trace resources modified: 46
  Total evaluation time: 10s 100ms
----------------------------------------------------------------------
```

The first argument is the trace file to transform; the second argument is the SKEL file to apply.  An optional output
file is specified with the `-o` flag; if not specified, the output will be written to the input filename with a
modification timestamp appended.  See the [skctl CLI reference](../components/skctl.md) for all options to the `skctl
transform` command.
