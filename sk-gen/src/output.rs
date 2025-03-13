// use std::collections::BTreeMap;
// use std::path::PathBuf;

// use anyhow::Result;
// use kube::api::DynamicObject;
// use sk_store::{
//     TraceEvent,
//     TraceStorable,
//     TraceStore,
// };

// use crate::{
//     Cli,
//     ClusterGraph,
//     DynamicObjectNewType,
//     K8sObject,
//     Node,
//     Walk,
// };

// pub(crate) fn gen_trace_event(ts: i64, prev: &Node, next: &Node) -> TraceEvent {
//     let mut applied_objs = Vec::new();
//     let mut deleted_objs = Vec::new();

//     for (name, object) in &prev.objects {
//         if !next.objects.contains_key(name) {
//             deleted_objs.push(object.current_state().dynamic_object);
//         } else if object.current_state().dynamic_object != next.objects[name].current_state().dynamic_object {
//             applied_objs.push(next.objects[name].current_state().dynamic_object);
//         }
//     }

//     for (name, object) in &next.objects {
//         if !prev.objects.contains_key(name) {
//             applied_objs.push(object.current_state().dynamic_object);
//         }
//     }

//     let applied_objs: Vec<DynamicObject> =
//         applied_objs.into_iter().map(|obj_wrapper| obj_wrapper.dynamic_object).collect();
//     let deleted_objs: Vec<DynamicObject> = deleted_objs
//         .into_iter()
//         .map(|obj_wrapper| obj_wrapper.current_state().dynamic_object)
//         .collect();

//     TraceEvent { ts, applied_objs, deleted_objs }
// }

// pub(crate) fn display_walks_and_traces(walks: &[Walk], traces: &[TraceStore], cli: &Cli) -> Result<()> {
//     if let Some(traces_dir) = &cli.traces_output_dir {
//         if !traces_dir.exists() {
//             std::fs::create_dir_all(traces_dir)?;
//         }
//     }

//     if cli.display_walks {
//         println!("num walks: {}", walks.len());
//     }

//     for (i, (walk, trace)) in walks.iter().zip(traces.iter()).enumerate() {
//         let min_ts = trace.start_ts().unwrap();
//         let max_ts = trace.end_ts().unwrap() + 1;

//         let export_filters = sk_api::v1::ExportFilters::default();
//         if let Some(traces_dir) = &cli.traces_output_dir {
//             let data = trace.export(min_ts, max_ts, &export_filters)?;
//             let path = traces_dir.join(format!("trace-{i}.mp"));
//             std::fs::write(path, data)?;
//         }

//         if cli.display_walks {
//             println!("walk-{i}:");
//             display_walk(walk);
//             println!();
//         }
//     }

//     Ok(())
// }

// fn display_walk(walk: &Walk) {
//     for (edge, node) in walk {
//         if let Some(e) = edge {
//             println!("{:#?}", e.action);
//         }
//         println!("{node:#?}");
//     }
// }

// pub(crate) fn export_graphviz(graph: &ClusterGraph, output_file: &PathBuf) -> Result<()> {
//     assert!(!output_file.is_dir(), "graph output file must not be a directory");

//     if let Some(parent) = output_file.parent() {
//         if !parent.exists() {
//             std::fs::create_dir_all(parent)?;
//         }
//     }
//     std::fs::write(output_file, graph.to_graphviz())?;
//     Ok(())
// }

// pub(crate) fn write_debug_info(
//     candidate_objects: &BTreeMap<String, DynamicObjectNewType>,
//     nodes: &[Node],
//     output_dir: &PathBuf,
// ) -> Result<()> {
//     std::fs::create_dir_all(output_dir)?;

//     std::fs::write(output_dir.join("candidate_objects.json"), serde_json::to_string_pretty(&candidate_objects)?)?;

//     for (i, node) in nodes.iter().enumerate() {
//         std::fs::write(output_dir.join(format!("node-{i}.ron")), format!("{:#?}", node))?;
//     }

//     Ok(())
// }
