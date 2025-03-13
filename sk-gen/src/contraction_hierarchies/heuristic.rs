use petgraph::graph::{Graph, NodeIndex};
use std::collections::HashMap;

pub fn nested_dissection_contraction_order<N, E>(graph: Graph<N, E>) -> Vec<NodeIndex> {
    let heuristic_graph = HeuristicGraph { graph };
    heuristic_graph.contraction_order()
}

pub struct HeuristicGraph<N, E> {
    pub graph: Graph<N, E>,
}

impl<N, E> HeuristicGraph<N, E> {
    pub fn new(graph: Graph<N, E>) -> Self {
        Self { graph }
    }

    pub fn contraction_order(&self) -> Vec<NodeIndex> {
        self.generate_partition_tree(self.graph.node_indices().collect())
            .to_vec()
    }

    pub fn generate_partition_tree(&self, nodes: Vec<NodeIndex>) -> PartitionTreeNode {
        if nodes.len() <= 1 {
            return PartitionTreeNode {
                a: None,
                b: None,
                separator: nodes,
            };
        }

        // Map each node to an index for METIS.
        let mut node_to_metis = HashMap::new();
        let mut metis_to_node = Vec::with_capacity(nodes.len());
        for (i, &node) in nodes.iter().enumerate() {
            node_to_metis.insert(node, i);
            metis_to_node.push(node);
        }

        // METIS input: CSR for the `nodes` induced subgraph.

        let mut xadj = Vec::with_capacity(nodes.len() + 1);
        let mut adjncy = Vec::new();
        let mut current_idx = 0;
        xadj.push(current_idx);
        for &node in &nodes {
            for neighbor in self.graph.neighbors(node) {
                if nodes.contains(&neighbor) {
                    let metis_idx = node_to_metis[&neighbor];
                    adjncy.push(metis_idx as metis::Idx);
                    current_idx += 1;
                }
            }
            xadj.push(current_idx);
        }

        // Partition the graph into a, b
        let mut part = vec![0; nodes.len()];
        let metis_graph =
            metis::Graph::new(1, 2, &xadj, &adjncy).expect("Failed to create METIS graph");
        metis_graph
            .part_recursive(&mut part)
            .expect("Failed to partition graph");

        let mut a = Vec::new();
        let mut b = Vec::new();
        for (i, &p) in part.iter().enumerate() {
            let node = metis_to_node[i];
            if p == 0 {
                a.push(node);
            } else {
                b.push(node);
            }
        }

        // TODO: The correct way
        // Let S be the minimal vertex cover of the edges with one endpoint in each of A,B.
        //
        // A' = A \ S
        // B' = B \ S
        // (A',B',S) is our dissection

        // HACK: Identify separator nodes as those in A that have neighbors in B.
        let mut separator = Vec::new();
        let mut is_separator = vec![false; nodes.len()];
        for &node in &a {
            for neighbor in self.graph.neighbors(node) {
                if nodes.contains(&neighbor) && part[node_to_metis[&neighbor]] == 1 {
                    is_separator[node_to_metis[&node]] = true;
                    break;
                }
            }
        }
        a.retain(|&node| {
            if is_separator[node_to_metis[&node]] {
                separator.push(node);
                false
            } else {
                true
            }
        });

        // Recurse
        let left = self.generate_partition_tree(a);
        let right = self.generate_partition_tree(b);

        PartitionTreeNode {
            a: Some(Box::new(left)),
            b: Some(Box::new(right)),
            separator,
        }
    }
}

pub struct PartitionTreeNode {
    a: Option<Box<PartitionTreeNode>>,
    b: Option<Box<PartitionTreeNode>>,
    separator: Vec<NodeIndex>,
}

impl PartitionTreeNode {
    /// Flattens the recursive ordering into a single vector.
    pub fn to_vec(&self) -> Vec<NodeIndex> {
        let mut order = Vec::new();
        if let Some(ref left) = self.a {
            order.extend(left.to_vec());
        }
        if let Some(ref right) = self.b {
            order.extend(right.to_vec());
        }
        order.extend(&self.separator);
        order
    }
}
