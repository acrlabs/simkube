#![feature(let_chains)]

use ordered_float::Float;

use anyhow::{Context, Result};
use dijkstra::dijkstra;
use ordered_float::OrderedFloat;
use petgraph::{graph::NodeIndex, Graph};
use std::hash::Hash;
use petgraph::visit::EdgeRef;

/// A wrapper on Node which lets us mark a node as contracted with respect to a particular iteration.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum CHNode<Node> {
    Original { node: Node },
    Contracted { node: Node, iteration: usize },
}

impl<Node> CHNode<Node> {
    fn new_original(node: Node) -> Self {
        CHNode::Original { node }
    }
}

/// A wrapper on Node which lets us mark a node as contracted with respect to a particular iteration.
#[derive(Clone)]
pub enum CHEdge<Edge> {
    Original { edge: Edge },
    Shortcut { 
        edges: Vec<Edge>,  // Store edges instead of edge indices
        nodes: Vec<NodeIndex>,  // Store all intermediate nodes in the path
        iteration: usize 
    },
    Orphaned {
        edge: Edge,
        iteration: usize,  // Store which iteration this edge was orphaned in
    },
}

impl<Edge> CHEdge<Edge> {
    fn new_original(edge: Edge) -> Self {
        CHEdge::Original { edge }
    }
}

// Custom Debug implementation for CHEdge to show probability of shortcut
impl<Edge: std::fmt::Debug + Distance> std::fmt::Debug for CHEdge<Edge> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CHEdge::Original { edge } => {
                f.debug_struct("Original")
                    .field("edge", edge)
                    .finish()
            }
            CHEdge::Shortcut { edges, nodes, iteration } => {
                let probability = self.probability().into_inner();
                
                f.debug_struct("Shortcut")
                    .field("edges", edges)
                    .field("nodes", nodes)
                    .field("iteration", iteration)
                    .field("probability", &probability)
                    .finish()
            }
            CHEdge::Orphaned { edge, iteration } => {
                f.debug_struct("Orphaned")
                    .field("edge", edge)
                    .field("iteration", iteration)
                    .finish()
            }
        }
    }
}

// TODO: Analyze the ways in which OrderedFloat breaks IEEE754 to provide ordering, and validate that such breaks from the standard to not cause problems where we are using
pub trait Distance {
    fn probability(&self) -> OrderedFloat<f64>;

    fn surprisal(&self) -> OrderedFloat<f64> {
        -self.probability().ln()
    }
}

impl<E: Distance> Distance for CHEdge<E> {
    fn probability(&self) -> OrderedFloat<f64> {
        match self {
            CHEdge::Original { edge } => edge.probability(),
            CHEdge::Shortcut { edges, nodes: _, iteration: _ } => {
                edges.iter()
                    .map(|edge| edge.probability())
                    .product()
            }
            CHEdge::Orphaned { edge: _, iteration: _ } => OrderedFloat::from(0.0),
        }
    }
}

impl Distance for () {
    fn probability(&self) -> OrderedFloat<f64> {
        OrderedFloat(1.0)
    }
}

pub trait ContractionHeuristic<N, E> {
    // Only call after the previously specified returned node (if any) has been contracted
    fn next_contraction(&mut self, graph: &Graph<CHNode<N>, CHEdge<E>>) -> Option<NodeIndex>;
}

impl<I, N, E> ContractionHeuristic<N, E> for I
where
    I: Iterator<Item = NodeIndex>,
{
    fn next_contraction(&mut self, _graph: &Graph<CHNode<N>, CHEdge<E>>) -> Option<NodeIndex> {
        self.next()
    }
}

type SurprisalType = ordered_float::OrderedFloat<f64>; // TODO make generic over distance types

#[derive(Clone)]
struct SearchResult {
    surprisal: SurprisalType,
    path: Vec<NodeIndex>,
}

// Y-Statement ADR on the design of CH, CHNode, and CHEdge:
// In the context of designing a type to represent the state of the Contraction Hierarchies algorithm, where the output graph--the eponymous "contraction hierarchy"-- is the union of all intermediate states--"core graphs",
// facing the need to minimize memory footprint while retaining conceptual simplicity,
// we decided for a representation which wraps the original Node and Edge types to optionally mark each as contracted or as a shortcut during a particular iteration on a single continuously increasing graph
// and neglected recording each core graph separately--where contracted nodes are deleted and shortcuts indistiguishable from original edges--
// to achieve the reduced memory footprint of a single representation from which both all prior core graphs and the final contraction hierarchy can be cheaply computed,
// accepting the increased cost during each witness search of having to check (and subsequently skip) edges to contracted nodes,
// because this cost is bounded by the initial degrees of each node, which are constant, and other mitigating steps (such as storing edges to contracted nodes separately or later in the adjacency list) are concievable

// Remove IntoIterator complexity and just use a generic type H that implements ContractionHeuristic
#[derive(Clone)]
pub struct CH<N, E, H>
where
    E: Distance,
    H: ContractionHeuristic<N, E>,
{
    graph: Graph<CHNode<N>, CHEdge<E>>,
    heuristic: H,
    num_contractions: usize,
}

use std::fmt::Debug;

impl<N: Clone + Hash + Eq + Debug, E: Clone + Hash + Debug, H> CH<N, E, H>
where
    N: Clone + Hash + Eq + Debug,
    E: Clone + Hash + Debug + Distance,
    H: ContractionHeuristic<N, E>,
{
    fn annotate_graph(graph: Graph<N, E>) -> Graph<CHNode<N>, CHEdge<E>> {
        graph.map(
            |_, n| CHNode::new_original(n.clone()),
            |_, e| CHEdge::new_original(e.clone()),
        )
    }

    pub fn new(graph: Graph<N, E>, heuristic: H) -> Self {
        let graph = Self::annotate_graph(graph);

        Self {
            graph,
            heuristic,
            num_contractions: 0,
        }
    }

    fn g_distance(&self, x_index: NodeIndex, y_index: NodeIndex) -> Option<SearchResult> {
        let (distances, predecessors) = dijkstra(&self.graph, x_index, Some(y_index), |e| {
            use petgraph::visit::EdgeRef;
            // Skip edges that connect to contracted nodes or are orphaned
            match (&self.graph[e.source()], &self.graph[e.target()], &self.graph[e.id()]) {
                (CHNode::Contracted { .. }, _, _) | (_, CHNode::Contracted { .. }, _) => {
                    OrderedFloat::nan()
                }
                (_, _, CHEdge::Orphaned { .. }) => {
                    OrderedFloat::nan() // Skip orphaned edges too
                }
                _ => e.weight().surprisal(),
            }
        });

        // Reconstruct path from predecessors map
        let mut path = Vec::new();
        let mut current = y_index;
        path.push(current);

        while let Some(&prev) = predecessors.get(&current) {
            path.push(prev);
            current = prev;
            if current == x_index {
                break;
            }
        }
        path.reverse();

        // Only return Some if the distance is less than infinity (meaning a valid path was found)
        distances.get(&y_index).and_then(|&distance| {
            // TODO validate this use of ordered float is correct
            if distance < OrderedFloat::infinity() {
                Some(SearchResult {
                    surprisal: distance,
                    path,
                })
            } else {
                None
            }
        })
    }

    fn g_distance_limited(
        &self,
        x: NodeIndex,
        y: NodeIndex,
        limit: SurprisalType,
    ) -> Option<SearchResult> {
        // TODO actually use limit
        self.g_distance(x, y)
    }

    // Weighted vertex contraction of a vertex v in the graph G is defined as the operation of removing v
    // and inserting (a minimum number of shortcuts) among the neighbors of v to
    // obtain a graph G′ such that distG(x, y) = distG′ (x, y) for all vertices x !=
    // v and y != v.
    fn contract(&mut self, node_index: NodeIndex) -> Result<()> {
        // "To compute G′, one iterates over all pairs of neighbors x, y of v increasing by distG(x, y)."
        use petgraph::Direction;
        use petgraph::visit::EdgeRef;

        // First, collect all incident edges before we change any of them
        let incoming_edges: Vec<_> = self.graph
            .edges_directed(node_index, Direction::Incoming)
            .map(|edge| (edge.id(), edge.source()))
            .collect();
            
        let outgoing_edges: Vec<_> = self.graph
            .edges_directed(node_index, Direction::Outgoing)
            .map(|edge| (edge.id(), edge.target()))
            .collect();

        let out_neighbors = self
            .graph
            .neighbors_directed(node_index, Direction::Outgoing)
            .filter(|&n| !matches!(self.graph[n], CHNode::Contracted { .. }));
        let in_neighbors = self
            .graph
            .neighbors_directed(node_index, Direction::Incoming)
            .filter(|&n| !matches!(self.graph[n], CHNode::Contracted { .. }));

        use itertools::Itertools;

        // First collect all pairs and their original distances before contraction
        let in_out_pairs: Vec<_> = Itertools::cartesian_product(in_neighbors, out_neighbors)
            .map(|(x, y)| {
                (
                    x,
                    y,
                    self.g_distance(x, y)
                        .with_context(|| {
                            format!(
                                "Failed to compute distance between {:?} and {:?} on graph {:#?}",
                                x, y, self.graph
                            )
                        })
                        .unwrap()
                        .surprisal,
                )
            })
            .sorted_by_key(|(_, _, d)| *d)
            .collect();

        // Now mark the node as contracted
        eprintln!(
            "Starting contraction of node {node_index:?} (iteration {})",
            self.num_contractions
        );
        match &self.graph[node_index] {
            CHNode::Original { node } => {
                self.graph[node_index] = CHNode::Contracted {
                    node: node.clone(),
                    iteration: self.num_contractions,
                };
                println!(
                    "Contracted node {node_index:?} in iteration {}: {:?}",
                    self.num_contractions, self.graph[node_index]
                );
            }
            CHNode::Contracted { .. } => {
                panic!("Attempted to contract node {node_index:?} which is already contracted");
            }
        }

        // Mark all incident edges as orphaned
        for (edge_id, _) in &incoming_edges {
            if let CHEdge::Original { edge } = self.graph[*edge_id].clone() {
                self.graph[*edge_id] = CHEdge::Orphaned {
                    edge,
                    iteration: self.num_contractions,
                };
                println!("Marked incoming edge {:?} as orphaned in iteration {}", 
                    edge_id, self.num_contractions);
            }
        }
        
        for (edge_id, _) in &outgoing_edges {
            if let CHEdge::Original { edge } = self.graph[*edge_id].clone() {
                self.graph[*edge_id] = CHEdge::Orphaned {
                    edge,
                    iteration: self.num_contractions,
                };
                println!("Marked outgoing edge {:?} as orphaned in iteration {}", 
                    edge_id, self.num_contractions);
            }
        }

        // witness search -- i.e. does removing v destroy the previously existing shortest path between x and y?
        // TODO: Shortcut should probability sum over all path lengths to preserve stochastic transition probabilities
        //       There may be a better algorithm for "find probability of all probability-weighted paths from A->C via B"

        for (x, y, d) in in_out_pairs {
            // TODO: We probably need to search the whole graph to avoid looking at any paths
            let search_result = self.g_distance_limited(x, y, d * 2.0);

            let should_add_shortcut = match search_result.clone() {
                Some(result) if result.surprisal <= d => {
                    println!("Found witness path from {x:?} to {y:?}: {:?} with distance {} (original distance: {})", 
                        result.path, result.surprisal, d);
                    false
                }
                Some(result) => {
                    println!("Path found from {x:?} to {y:?} with distance {} > {} (original) - adding shortcut", 
                        result.surprisal, d);
                    true
                }
                None => {
                    println!("No path found between {x:?} and {y:?} - adding shortcut (original distance: {})", d);
                    true
                }
            };

            if should_add_shortcut {
                // Find the probability of the path through the contracted node
                // This is the "destroyed" probability we want to capture
                let mut destroyed_edges = Vec::new();
                
                // Get edge from x to node_index if it exists
                if let Some(in_edge_idx) = self.graph.find_edge(x, node_index) {
                    match &self.graph[in_edge_idx].clone() {
                        CHEdge::Original { edge } => destroyed_edges.push(edge.clone()),
                        CHEdge::Orphaned { edge, .. } => destroyed_edges.push(edge.clone()),
                        _ => {}
                    }
                }
                
                // Get edge from node_index to y if it exists
                if let Some(out_edge_idx) = self.graph.find_edge(node_index, y) {
                    match &self.graph[out_edge_idx].clone() {
                        CHEdge::Original { edge } => destroyed_edges.push(edge.clone()),
                        CHEdge::Orphaned { edge, .. } => destroyed_edges.push(edge.clone()),
                        _ => {}
                    }
                }
                
                // Add the shortcut edge to represent the destroyed path
                self.graph.add_edge(
                    x,
                    y,
                    CHEdge::Shortcut {
                        edges: destroyed_edges,
                        nodes: vec![x, node_index, y],
                        iteration: self.num_contractions,
                    },
                );
                println!(
                    "Added shortcut edge from {x:?} to {y:?} in iteration {} for destroyed path",
                    self.num_contractions
                );
            }
        }
        self.num_contractions += 1;
        Ok(())
    }

    fn contract_to(&mut self, iteration: usize) -> Result<&mut Self> {
        while self.num_contractions < iteration {
            let next_contraction = self
                .heuristic
                .next_contraction(&self.graph)
                .context("No more contractions to perform")?;
            self.contract(next_contraction)?;
        }
        Ok(self)
    }


    pub fn core_graph(&mut self, i: usize) -> Result<Graph<CHNode<N>, CHEdge<E>>>
    where
        N: Clone,
        E: Clone,
    {
        self.contract_to(i)?;

        Ok(self.graph.filter_map(
            |_, n| match n {
                CHNode::Original { node } => Some(CHNode::Original { node: node.clone() }),
                CHNode::Contracted { node, iteration } => {
                    if *iteration > i {
                        // TODO check off-by-one
                        Some(CHNode::Original { node: node.clone() })
                    } else {
                        Some(n.clone())
                    }
                }
            },
            |_, e| match e {
                CHEdge::Original { .. } => Some(e.clone()),
                CHEdge::Shortcut { edges, iteration, .. } => {
                    if *iteration <= i {
                        // TODO check off-by-one
                        Some(e.clone())
                    } else {
                        None
                    }
                }
                CHEdge::Orphaned { .. } => Some(e.clone()), // Include orphaned edges in the core graph
            },
        ))
    }

    pub fn contraction_hierarchy(&mut self) -> Result<Graph<CHNode<N>, CHEdge<E>>> {
        while let Some(next_contraction) = self.heuristic.next_contraction(&self.graph) {
            self.contract(next_contraction)?;
        }

        Ok(self.graph.filter_map(
            |_, n| match n {
                CHNode::Original { node } | CHNode::Contracted { node, .. } => {
                    Some(CHNode::Original { node: node.clone() })
                }
            },
            |_, e| Some(e.clone()), // Include all edges, including orphaned edges
        ))
    }
}


// Add a trait for computing probability from a graph and edge index
pub trait GraphDistance<E: Distance> {
    fn edge_probability(&self, edge_idx: petgraph::prelude::EdgeIndex) -> OrderedFloat<f64>;
    fn path_probability(&self, edge_indices: &[petgraph::prelude::EdgeIndex]) -> OrderedFloat<f64>;
}

// Implement GraphDistance for any Graph with edge weights implementing Distance
impl<N, E: Distance> GraphDistance<E> for Graph<N, E> {
    fn edge_probability(&self, edge_idx: petgraph::prelude::EdgeIndex) -> OrderedFloat<f64> {
        self.edge_weight(edge_idx)
            .expect("Edge index should be valid")
            .probability()
    }
    
    fn path_probability(&self, edge_indices: &[petgraph::prelude::EdgeIndex]) -> OrderedFloat<f64> {
        edge_indices.iter()
            .map(|&idx| self.edge_probability(idx))
            .product()
    }
}


pub trait AssertStochastic {
    fn assert_stochastic(&self) -> bool;
}

impl<N, E: Distance> AssertStochastic for Graph<N, E> {
    fn assert_stochastic(&self) -> bool {
        // assert that all edges have a probability between 0 and 1, and sum to 1
        let mut sum = OrderedFloat::from(0.0);
        for edge in self.edge_references() {
            let probability = edge.weight().probability();
            if probability < OrderedFloat::from(0.0) || probability > OrderedFloat::from(1.0) {
                return false;
            }
            sum += probability;
        }
        if sum != OrderedFloat::from(1.0) {
            return false;
        }
        true
    }
}
