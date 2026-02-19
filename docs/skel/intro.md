<!--
template: docs.html
-->

# SKEL: The SimKube Expression Language

## What in SKEL?

The SimKube Expression Language (aka SKEL) is a bespoke DSL for querying and modifying events in a SimKube trace.  This
DSL allows SimKube users to clean, sanitize or obfuscate, or generate new scenarios from an existing trace file.  These
modifications can be defined in a simple text-based file format that lives alongside your trace files, checked in to
version control, and applied consistently across different simulation environments or configurations.

## Why do we need a DSL?

As described in the [trace file reference](../ref/trace-files.md), SimKube trace files are stored using the
[msgpack](https://msgpack.org/index.html) binary file format, which reduces the size of the stored trace files, but is
somewhat cumbersome to work with by hand.  Some tools exist for manually interacting with msgpack, but they typically
involve "converting the file to JSON, making edits by hand, and then converting back."  Unfortunately this doesn't work
for SimKube trace files because msgpack and JSON are not 1:1 equivalent and SimKube trace files use some msgpack
features that don't easily roundtrip.

A second option would be to just require SimKube users to modify SimKube trace files using a general-purpose programming
language like Python, along with the msgpack library interface for that language.  The downside to this approach is that
users either need to understand the internal schema for trace files (which may change in the future), or we need to
provide a SimKube-specific API for each language we wanted to support.  This approach provides the most flexibility to
SimKube users, but requires more complexity and maintenance in the long run.

The last option (and the one we chose) is to implement a DSL for SimKube that describes a subset of extremely common
operations that users might want to perform on a SimKube trace in a simple, easy-to-read and easy-to-maintain format.
We don't have to deal with the full complexity of arbitrary programming languages, and if the trace file format changes
in the future, that can be abstracted behind the DSL interpreter, so that users' `.skel` files continue to work.

## What alternatives are there?

Implementing a DSL is itself a non-trivial task; why does SimKube include its own DSL instead of leveraging some
pre-existing solution?  The main problem is that there aren't any other tools out there that understand the SimKube
trace format, so that almost by necessity forces a new language.  We did consider using the [Common Expression Language
(CEL)](https://cel.dev), which is used elsewhere in the Kubernetes ecosystem, but ran into two issues: a) CEL is
non-mutating, so we still need (at least) a CEL extension to support mutating operations on SimKube traces, and b) most
of the nice Kubernetes-specific CEL extensions are written in Golang, and the rest of SimKube is in Rust.  Given these
two factors, instead of making a separate Golang-specific tool that would still require extensive customization, we
instead just built our own DSL from scratch in Rust.  Time will tell if this was the right choice :)

*[DSL]: Domain Specific Language
