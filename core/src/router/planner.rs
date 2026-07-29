use chrono::Utc;
use petgraph::algo::astar;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

use crate::router::graph::{MapEdge, MapNode};
use crate::vda5050::{Edge, Header, Node, NodePosition, Order};

#[derive(Debug, Clone)]
pub struct RoutePlan {
    pub node_ids: Vec<String>,
    pub edge_ids: Vec<String>,
    pub total_distance_m: f64,
}

impl RoutePlan {
    pub fn into_vda5050_order(
        &self,
        order_id: &str,
        order_update_id: u32,
        manufacturer: &str,
        serial_number: &str,
        router: &TopologicalRouter, // <-- Pass the router here
    ) -> Order {
        let header = Header {
            header_id: Utc::now().timestamp_millis() as u32,
            timestamp: Utc::now().to_rfc3339(),
            version: "3.0.0".to_string(),
            manufacturer: manufacturer.to_string(),
            serial_number: serial_number.to_string(),
        };

        let mut nodes = Vec::with_capacity(self.node_ids.len());
        let mut edges = Vec::with_capacity(self.edge_ids.len());

        let mut current_seq = 0u32;

        for (i, node_id) in self.node_ids.iter().enumerate() {
            // Look up coordinates from the router graph
            let node_position = router.node_lookup.get(node_id).map(|&idx| {
                let map_node = &router.graph[idx];
                NodePosition {
                    x: map_node.x,
                    y: map_node.y,
                    theta: Some(0.0), // Default orientation
                    map_id: "map".to_string(),
                    map_description: None,
                }
            });

            nodes.push(Node {
                node_id: node_id.clone(),
                sequence_id: current_seq,
                node_description: None,
                released: true,
                node_position, // <-- Populated with real coordinates!
                actions: Vec::new(),
            });

            current_seq += 1;

            if i < self.edge_ids.len() {
                let edge_id = &self.edge_ids[i];
                let start_node_id = node_id.clone();
                let end_node_id = self.node_ids[i + 1].clone();

                edges.push(Edge {
                    edge_id: edge_id.clone(),
                    sequence_id: current_seq,
                    edge_description: None,
                    released: true,
                    start_node_id,
                    end_node_id,
                    max_speed: None,
                    actions: Vec::new(),
                });

                current_seq += 1;
            }
        }

        Order {
            header,
            order_id: order_id.to_string(),
            order_update_id,
            zone_set_id: None,
            nodes,
            edges,
        }
    }
}

pub struct TopologicalRouter {
    graph: DiGraph<MapNode, MapEdge>,
    node_lookup: HashMap<String, NodeIndex>,
}

impl TopologicalRouter {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_lookup: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, id: &str, x: f64, y: f64) {
        let node = MapNode {
            id: id.to_string(),
            x,
            y,
        };
        let idx = self.graph.add_node(node);
        self.node_lookup.insert(id.to_string(), idx);
    }

    pub fn add_edge(
        &mut self,
        id: &str,
        from_id: &str,
        to_id: &str,
        distance_m: f64,
    ) -> Result<(), String> {
        let &from_idx = self
            .node_lookup
            .get(from_id)
            .ok_or_else(|| format!("Start node '{from_id}' not found in map"))?;
        let &to_idx = self
            .node_lookup
            .get(to_id)
            .ok_or_else(|| format!("Target node '{to_id}' not found in map"))?;

        let edge = MapEdge {
            id: id.to_string(),
            distance_m,
        };

        self.graph.add_edge(from_idx, to_idx, edge);
        Ok(())
    }

    pub fn find_path(&self, start_id: &str, target_id: &str) -> Option<RoutePlan> {
        let &start_idx = self.node_lookup.get(start_id)?;
        let &target_idx = self.node_lookup.get(target_id)?;
        let target_node = &self.graph[target_idx];

        let path_opt = astar(
            &self.graph,
            start_idx,
            |finish| finish == target_idx,
            |e| (e.weight().distance_m * 1000.0) as u64,
            |n| {
                let current = &self.graph[n];
                let dx = current.x - target_node.x;
                let dy = current.y - target_node.y;
                ((dx * dx + dy * dy).sqrt() * 1000.0) as u64
            },
        );

        let (_total_cost_mm, path_indices) = path_opt?;

        let mut node_ids = Vec::with_capacity(path_indices.len());
        let mut edge_ids = Vec::with_capacity(path_indices.len().saturating_sub(1));
        let mut total_distance_m = 0.0;

        for window in path_indices.windows(2) {
            let from_idx = window[0];
            let to_idx = window[1];

            node_ids.push(self.graph[from_idx].id.clone());

            if let Some(edge_idx) = self.graph.find_edge(from_idx, to_idx) {
                let edge = &self.graph[edge_idx];
                edge_ids.push(edge.id.clone());
                total_distance_m += edge.distance_m;
            }
        }

        if let Some(&last_idx) = path_indices.last() {
            node_ids.push(self.graph[last_idx].id.clone());
        }

        Some(RoutePlan {
            node_ids,
            edge_ids,
            total_distance_m,
        })
    }
}
