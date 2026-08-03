use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(crate) struct MapNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct MapEdge {
    pub id: String,
    pub distance_m: f64,
}

#[derive(Deserialize)]
pub(crate) struct NodePos {
    pub x: f64,
    pub y: f64,
}

#[derive(Deserialize)]
pub(crate) struct GraphConfig {
    pub nodes: HashMap<String, NodePos>,
    pub edges: Vec<(String, String, f64)>, // [from_node, to_node, distance_m]
}