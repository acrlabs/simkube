<!--
template: ref.html
-->

# Simulation CRD Spec (cluster-scoped)

- Name: simulations.simkube.io
- Group: simkube.io
- Version: v1
- Kind: Simulation

## Schema
<div class="schema" markdown>
/// details | `apiVersion`: `string`
APIVersion defines the versioned schema of this representation of an object. Servers should convert recognized
schemas to the latest internal value, and may reject unrecognized values. More info:
https://git.k8s.io/community/contributors/devel/sig-architecture/api-conventions.md#resources
///

/// details | `kind`: `string`
Kind is a string value representing the REST resource this object represents. Servers may infer this from the
endpoint the client submits requests to. Cannot be updated. In CamelCase. More info:
https://git.k8s.io/community/contributors/devel/sig-architecture/api-conventions.md#types-kinds
///

/// details | `metadata`: `ObjectMeta`
Standard object's metadata. More info:
https://git.k8s.io/community/contributors/devel/sig-architecture/api-conventions.md#metadata
///

<!-- SimulationSpec start -->
/// details | \*`spec`: `SimulationSpec`
Auto-generated derived type for `SimulationSpec` via `CustomResource`

<!-- SimulationDriverConfig start -->
/// details | \*`driver`: `SimulationDriverConfig`
Configuration for the simulation driver pod

/// details | `args`: `[]string`
Additional argument values to pass into `sk-driver`
///

/// details | \*`image`: `string`
Docker image to use for the driver pod
///

/// details | \*`namespace`: `string`
Namespace to launch the driver job in
///

/// details | \*`tracePath`: `string`
Location of the trace file for the driver to run, prefixed by a scheme; the scheme can be one of `file://`,
`s3://`, `azure://`, or `gs://` for local storage, AWS S3, Azure Blob Store, or Google Cloud Storage,
respectively.  Local trace files _must_ be present on the Kubernetes node where the driver is running, and the
path must be the path to the trace file on the node.
///
///
<!-- SimulationDriverConfig end -->

/// details | `duration`: `string`
Length of time to run the simulation for; can be a variety of human-parseable formats, e.g., "5m".  If not
specified, the simulation will run until the last event in the trace file.  Can be used to cut a simulation
short, or to extend a simulation psat the last event in the trace.
///

<!-- SimulationHooksConfig start -->
/// details | `hooks`: `SimulationHooksConfig`
Hooks are simple commands that can be run before or after simulations or simulation iterations have completed.

<!-- SimulationHook start -->
/// details | `postRunHooks`: `[]SimulationHook`
`postRunHooks` execute after each iteration of the simulation is complete.  Post-run hooks are executed
inside the Simulation driver pod.

/// details | `args`: `[]string`
Additional arguments that should be passed to the hook command
///

/// details |\*`cmd`: `string`
The command that should be executed
///

/// details | `ignoreFailure`: `boolean`
Whether the simulation should be aborted if the hook fails
///

/// details | `sendSim`: `boolean`
If true, the Simulation object will be piped (as a JSON string) into the stdin of the hook process
///
///
<!-- SimulationHook end-->

<!-- SimulationHook start -->
/// details | `postStopHooks`: `[]SimulationHook`
`postStopHooks` execute once at the very end of a simulation run, after the last iteration of the simulation
is complete.  Post-stop hooks are executed inside the Simulation controller pod.

/// details | `args`: `[]string`
Additional arguments that should be passed to the hook command
///

/// details |\*`cmd`: `string`
The command that should be executed
///

/// details | `ignoreFailure`: `boolean`
Whether the simulation should be aborted if the hook fails
///

/// details | `sendSim`: `boolean`
If true, the Simulation object will be piped (as a JSON string) into the stdin of the hook process
///
///
<!-- SimulationHook end-->

<!-- SimulationHook start -->
/// details | `preRunHooks`: `[]SimulationHook`
`preRunHooks` execute before every iteration of the simulation starts.  Pre-run hooks are executed
inside the Simulation driver pod.

/// details | `args`: `[]string`
Additional arguments that should be passed to the hook command
///

/// details |\*`cmd`: `string`
The command that should be executed
///

/// details | `ignoreFailure`: `boolean`
Whether the simulation should be aborted if the hook fails
///

/// details | `sendSim`: `boolean`
If true, the Simulation object will be piped (as a JSON string) into the stdin of the hook process
///
///
<!-- SimulationHook end-->

<!-- SimulationHook start -->
/// details | `preStartHooks`: `[]SimulationHook`
`preStartHooks` execute once at the very beginnning of a simulation run, before the first iteration of the
simulation starts.  Pre-start hooks are executed inside the Simulation controller pod.

/// details | `args`: `[]string`
Additional arguments that should be passed to the hook command
///

/// details |\*`cmd`: `string`
The command that should be executed
///

/// details | `ignoreFailure`: `boolean`
Whether the simulation should be aborted if the hook fails
///

/// details | `sendSim`: `boolean`
If true, the Simulation object will be piped (as a JSON string) into the stdin of the hook process
///
///
<!-- SimulationHook end -->
///
<!-- SimulationHooksConfig end -->

<!-- SimulationMetricsConfig start -->
/// details | `metrics`: `SimulationMetricsConfig`
Configuration for exporting metrics from the simulation via Prometheus (requires the kube-prometheus stack
installed in the cluster).

/// details | `namespace`: `string`
What namespace to create the simulation Prometheus object in
///

/// details | `podMonitorNames`: `[]string`
An array of `PodMonitor` objects to watch.  More info:
https://prometheus-operator.dev/docs/api-reference/api/#monitoring.coreos.com/v1.PodMonitor
///

/// details | `podMonitorNamespaces`: `[]string`
A list of namespaces to search for pod monitors in
///

/// details | `prometheusShards`: `integer`
Number of Prometheus pods to run
///

/// details |\*`remoteWriteConfigs`: `RemoteWriteSpec`
Configuration for a remote write endpoint for the simulation Prometheus object to forward metrics to.
More info: https://prometheus-operator.dev/docs/api-reference/api/#monitoring.coreos.com/v1.RemoteWriteSpec
///

/// details | `serviceAccount`: `string`
The Kubernetes ServiceAccount the Prometheus pods should use.
More info: https://kubernetes.io/docs/tasks/configure-pod-container/configure-service-account/
///

/// details | `serviceMonitorNames`: `[]string`
An array of `ServiceMonitor` objects to watch.  More info:
https://prometheus-operator.dev/docs/api-reference/api/#monitoring.coreos.com/v1.ServiceMonitor
///

/// details | `serviceMonitorNamespaces`: `[]string`
A list of namespaces to search for service monitors in
///
///
<!-- SimulationMetricsConfig end -->

/// details | `pausedTime`: `datetime`
The time the simulation was last paused
///

/// details | `repetitions`: `integer`
The number of iterations the Simulation should perform, aka the number of times the simulation will repeat.
Specifically, the Simulation controller will create the Driver job with `parallelism = 1` and
`completions = self.spec.repetitions`.   Any configured `preRun` and `postRun` hooks will execute at the
start and end of each iteration, respectively.
///

/// details | `speed`: `double`
A multiplicative factor that will be applied to the interarrival time between events in the Simulation.  A
speed factor of `2.0` means that events from the trace will be applied twice as quickly.  Note: there is a
limit to how fast a simulation can be sped up, based on the response times of the Kubernetes control plane
and other controllers in the system.
///

///
<!-- SimulationSpec end -->

<!-- SimulationStatus start -->
/// details | `status`: `SimulationStatus`
Most recently observed status of the Simulation.  This data may not be up to date, and is populated by the
Simulation controller.

/// details | `completedRuns`: `unsigned integer`
Number of completed iterations (repetitions) of the Simulation
///

/// details | `endTime`: `datetime`
The completion time of the Simulation
///

/// details |\*`observedGeneration`: `integer`
Last observed "generation" of the Simulation custom resource; for internal use.
///

/// details | `startTime`: `datetime`
The start time of the Simulation
///

/// details | `state`: `enum`
The current state of the Simulation; one of `Blocked`, `Initializing`, `Finished`, `Failed`, `Paused`,
`Retrying`, or `Running`.
///

///
<!-- SimulationStatus end -->
</div>

# SimulationRoot CRD Spec (cluster-scoped)

- Name: simulationroots.simkube.io
- Group: simkube.io
- Version: v1
- Kind: SimulationRoot

## Schema

<div class="schema" markdown>
/// details | `apiVersion`: `string`
APIVersion defines the versioned schema of this representation of an object. Servers should convert recognized
schemas to the latest internal value, and may reject unrecognized values. More info:
https://git.k8s.io/community/contributors/devel/sig-architecture/api-conventions.md#resources
///

/// details | `kind`: `string`
Kind is a string value representing the REST resource this object represents. Servers may infer this from the
endpoint the client submits requests to. Cannot be updated. In CamelCase. More info:
https://git.k8s.io/community/contributors/devel/sig-architecture/api-conventions.md#types-kinds
///

/// details | `metadata`: `ObjectMeta`
Standard object's metadata. More info:
https://git.k8s.io/community/contributors/devel/sig-architecture/api-conventions.md#metadata
///

/// details |\*`spec`: `SimulationSpec`
Auto-generated derived type for SimulationSpec via `CustomResource` (always empty)
///
</div>
