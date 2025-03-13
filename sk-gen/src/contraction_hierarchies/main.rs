#![allow(dead_code)]
// use contraction_hierarchies::nested_dissection::nested_dissection_contraction_order;

// use contraction_hierarchies::CH;
// use contraction_hierarchies::Distance;
// use ordered_float::OrderedFloat;
// use petgraph::Graph;

// #[derive(Clone, Debug, Hash, Eq, PartialEq)]
// struct Edge {
//     weight: OrderedFloat<f64>,
// }

// impl Distance for Edge {
//     fn probability(&self) -> OrderedFloat<f64> {
//         self.weight
//     }
// }

// fn main() {
//     let mut graph = Graph::<(), Edge>::new();

//     // Create 3 nodes in a path: 0 -> 1 -> 2
//     let n0 = graph.add_node(());
//     let n1 = graph.add_node(());
//     let n2 = graph.add_node(());

//     graph.add_edge(n0, n1, Edge { weight: OrderedFloat::from(1.0) });
//     graph.add_edge(n1, n2, Edge { weight: OrderedFloat::from(1.0) });

//     use petgraph::prelude::NodeIndex;
//     let contraction_order = [1].map(NodeIndex::new).into_iter();

//     println!("Starting graph:\n{:#?}", graph);
//     let mut ch = CH::new(graph, contraction_order);

//     let core_graph = ch.core_graph(1);
//     let contraction_hierarchy = ch.contraction_hierarchy();

//     println!("core graph:\n{:#?}", core_graph);
//     println!("contraction hierarchy:\n{:#?}", contraction_hierarchy);


//     // 4 node diamond DAG
//     let mut graph = Graph::<(), Edge>::new();
//     let n0 = graph.add_node(());
//     let n1 = graph.add_node(());
//     let n2 = graph.add_node(());
//     let n3 = graph.add_node(());

//     // 0 -> 1 
//     // |    |
//     // v    v
//     // 2 -> 3

//     graph.add_edge(n0, n1, Edge { weight: OrderedFloat::from(0.5) });
//     graph.add_edge(n0, n2, Edge { weight: OrderedFloat::from(0.5) });
//     graph.add_edge(n1, n3, Edge { weight: OrderedFloat::from(1.0) });
//     graph.add_edge(n2, n3, Edge { weight: OrderedFloat::from(1.0) });

//     let contraction_order = [1, 2].map(NodeIndex::new).into_iter();
//     let mut ch = CH::new(graph, contraction_order);
//     let core_graph = ch.core_graph(2);
//     let contraction_hierarchy = ch.contraction_hierarchy();

//     println!("core graph:\n{:#?}", core_graph);
//     println!("contraction hierarchy:\n{:#?}", contraction_hierarchy);

//     // "Kite": 4 node diamond DAG + extra tail
//     let mut graph = Graph::<(), Edge>::new();
//     let n0 = graph.add_node(());
//     let n1 = graph.add_node(());
//     let n2 = graph.add_node(());
//     let n3 = graph.add_node(());
//     let n4 = graph.add_node(());

//     // 0 -> 1 
//     // |    |
//     // v    v
//     // 2 -> 3 -> 4

//     graph.add_edge(n0, n1, Edge { weight: OrderedFloat::from(0.5) });
//     graph.add_edge(n0, n2, Edge { weight: OrderedFloat::from(0.5) });
//     graph.add_edge(n1, n3, Edge { weight: OrderedFloat::from(1.0) });
//     graph.add_edge(n2, n3, Edge { weight: OrderedFloat::from(1.0) });
//     graph.add_edge(n3, n4, Edge { weight: OrderedFloat::from(1.0) });

//     let contraction_order = [3].map(NodeIndex::new).into_iter();
//     let mut ch = CH::new(graph, contraction_order);
//     let core_graph = ch.core_graph(1);
//     let contraction_hierarchy = ch.contraction_hierarchy();

//     println!("core graph:\n{:#?}", core_graph);
//     println!("contraction hierarchy:\n{:#?}", contraction_hierarchy);


//     // "cross"

//     //      0
//     //      |
//     //      V
//     // 1 -> 2 -> 3
//     //      |
//     //      V
//     //      4

//     let mut graph = Graph::<(), Edge>::new();
//     let n0 = graph.add_node(());
//     let n1 = graph.add_node(());
//     let n2 = graph.add_node(());
//     let n3 = graph.add_node(());
//     let n4 = graph.add_node(());

//     graph.add_edge(n0, n2, Edge { weight: OrderedFloat::from(1.0) });
//     graph.add_edge(n1, n2, Edge { weight: OrderedFloat::from(1.0) });

//     graph.add_edge(n2, n3, Edge { weight: OrderedFloat::from(0.5) });
//     graph.add_edge(n2, n4, Edge { weight: OrderedFloat::from(0.5) });

//     let contraction_order = [2].map(NodeIndex::new).into_iter();
//     let mut ch = CH::new(graph, contraction_order);
//     let core_graph = ch.core_graph(1);
//     let contraction_hierarchy = ch.contraction_hierarchy();

//     println!("core graph:\n{:#?}", core_graph);
//     println!("contraction hierarchy:\n{:#?}", contraction_hierarchy);
    
    

//     let n_middle = 2;
//     let mut graph = Graph::<(), Edge>::new();
//     let start = graph.add_node(());
//     let middle: Vec<NodeIndex> = (0..n_middle).map(|_| graph.add_node(())).collect();
//     let end = graph.add_node(());

//     let prob_start_to_middle = 1.0/n_middle as f64;

//     for i in 0..n_middle {
//         graph.add_edge(start, middle[i], Edge { weight: OrderedFloat::from(prob_start_to_middle) });
//         graph.add_edge(middle[i], end, Edge { weight: OrderedFloat::from(1.0) });
//     }

//     let mut ch = CH::new(graph, middle.into_iter());
//     let core_graph = ch.core_graph(n_middle);
//     let contraction_hierarchy = ch.contraction_hierarchy();

//     println!("core graph:\n{:#?}", core_graph);
//     println!("contraction hierarchy:\n{:#?}", contraction_hierarchy);


//     // bad case
//     // 0 -> 1
//     // |    |
//     // |-> 2
//     //     |
//     //     3

//     let mut graph =  Graph::<usize, Edge>::new();

//     let mut nodes: Vec<NodeIndex> = (0..4).map(|i| graph.add_node(i)).collect();

//     graph.add_edge(nodes[0], nodes[1], Edge { weight: OrderedFloat::from(0.2) });
//     graph.add_edge(nodes[0], nodes[2], Edge { weight: OrderedFloat::from(0.8) });

//     graph.add_edge(nodes[1], nodes[2], Edge { weight: OrderedFloat::from(1.0) });

//     graph.add_edge(nodes[2], nodes[3], Edge { weight: OrderedFloat::from(1.0) });


//     let contraction_order = [2].map(NodeIndex::new).into_iter();
//     let mut ch = CH::new(graph.clone(), contraction_order);
//     let core_graph = ch.core_graph(1).unwrap();
//     let contraction_hierarchy = ch.contraction_hierarchy().unwrap();
//     use contraction_hierarchies::AssertStochastic;
//     // assert!(core_graph.assert_stochastic());
//     // assert!(contraction_hierarchy.assert_stochastic());

//     println!("core graph:\n{:#?}", core_graph);
//     println!("contraction hierarchy:\n{:#?}", contraction_hierarchy);

//     use petgraph::dot::Dot;

//     let dot = Dot::new(&core_graph);
//     // write to file and run dot -Tpng <filename> > <outputfilename>.png
//     let mut file = std::fs::File::create("core_graph.dot").unwrap();
//     use std::io::Write;
//     file.write_all(format!("{:?}", dot).as_bytes()).unwrap();
    
//     // Fix the command by removing the > operator and saving output directly
//     let output = std::process::Command::new("dot")
//         .arg("-Tpng")
//         .arg("core_graph.dot")
//         .output()
//         .expect("Failed to execute dot command");
    
//     // Write the output to a file
//     if output.status.success() {
//         std::fs::write("core_graph.png", output.stdout).expect("Failed to write output image");
//         println!("Successfully created core_graph.png");
//     } else {
//         eprintln!("Error generating graph image: {}", String::from_utf8_lossy(&output.stderr));
//     }

//     // open the image
//     std::process::Command::new("open")
//         .arg("core_graph.png")
//         .output()
//         .unwrap();



//     // let mut graph = Graph::<(), Edge>::new();
//     // let mut nodes = Vec::new();
//     // let n = 5;
//     // for _ in 0..n {
//     //     nodes.push(graph.add_node(()));
//     // }

//     // // First, count the number of outgoing edges for each node
//     // let mut outgoing_edge_counts = vec![0; n];
//     // for (i, a) in nodes.iter().enumerate() {
//     //     // Count how many outgoing edges this node will have
//     //     let outgoing_count = nodes.iter().skip(i).take(2).count();
//     //     outgoing_edge_counts[i] = outgoing_count;
//     // }

//     // // Now add edges with evenly distributed probabilities
//     // for (i, a) in nodes.iter().enumerate() {
//     //     let outgoing_count = outgoing_edge_counts[i];
//     //     if outgoing_count > 0 {
//     //         // Calculate the probability for each outgoing edge
//     //         let probability = 1.0 / (outgoing_count as f64);
            
//     //         // Add the edges with the calculated probability
//     //         for b in nodes.iter().skip(i).take(2) {
//     //             let edge = Edge {
//     //                 weight: OrderedFloat::from(probability),
//     //             };
//     //             graph.add_edge(*a, *b, edge);
//     //         }
//     //     }
//     // }

//     // let contraction_order = nested_dissection_contraction_order(graph.clone()).into_iter();
//     // let mut ch = CH::new(graph, contraction_order.clone());

//     // let core_graph = ch.core_graph(contraction_order.len() - 1);
//     // let contraction_hierarchy = ch.contraction_hierarchy();
//     // println!("core graph:\n{:#?}", core_graph);
//     // println!("contraction hierarchy:\n{:#?}", contraction_hierarchy);
// }
