<!--
project: SimKube
template: docs.html
-->

# SimKube Concepts

SimKube is designed to allow users to simulate the behaviour of Kubernetes control plane components in a safe, isolated
local environment.  It is a "record-and-replay" simulator, which means that users can record the behaviour of a production
cluster and then save that data for future analysis.  Below we describe some of the key concepts of SimKube

## What components are simulated?

Typically when we talk about the Kubernetes control plane, we are talking about the API server, scheduler, and
controller manager.  SimKube expands this definition to include anything that can impact the behaviour of a cluster,
including projects like Cluster Autoscaler, descheduler, and others.

SimKube accomplishes this by running in a cluster with a real control plane; however, all pod behaviours are mocked out
using [Kubernetes WithOut Kubelet (KWOK)](https://kwok.sigs.k8s.io).  This means that anything that happens _inside_ a
pod is effectively out of scope of the simulation.  KWOK does have utilities to mock out some aspects of pod lifecycle,
but these are not (currently) supported by SimKube.  Crucially, this means that simulations that rely on the Horizontal
Pod Autoscaler (for example) will not currently work.

Note also that, unlike some simulation solutions, we are not mocking out any aspects of the control plane.  This means
that simulations of cluster behaviour take place in real-time, and we do not have any hooks into or control over what
messages are seen by various control plane components.  Thus, running the exact same simulation repeatedly may yield
different results on each run, depending on timing fluctuations and other challenges of distributed systems.

## How does it work?

SimKube has a number of components that it uses to record data and run simulations:

- _Tracer_: The `sk-tracer` program is a lightweight pod that runs in the cluster you wish to record the behaviour of.
  It saves cluster events into a in-memory event stream, that is, a timeline of "important" changes in the cluster.  You
  can configure `sk-tracer` to tell it what events you consider important.  Subsets of this event stream can be saved
  into a _trace file_, which can be replayed later in a simulated environment.
- _Controller_: The simulation controller `sk-ctrl` runs in a separate Kubernetes cluster and is responsible for setting
  up simulations in that cluster.  The separate cluster can be a local cluster running on your laptop using
  [kind](https://kind.sigs.k8s.io), or it can be a test cluster running in the cloud.  The only requirement is that the
  simulation must have all the components present that you wish to simulate.  The controller watches for `Simulation`
  custom resources to configure and start a new simulation run.  It sets up metrics collection and other required tools,
  and then creates a simulation driver Job, which actually reads the specified trace file and replays the events within
  against the simulated cluster.
- _CLI_: SimKube comes with an CLI utility called `skctl`, which can be used to export trace data from the cluster under
  study, as well as running new simulations in your simulated environment.
