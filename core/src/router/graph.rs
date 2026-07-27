#[derive(Debug, Clone)]
pub struct MapNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub struct MapEdge {
    pub id: String,
    pub distance_m: f64,
}