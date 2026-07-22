mod vda5050;

use vda5050::{BlockingType, Header, Node, NodePosition, Order};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 ISSEM FMS Core Initializing...");

    // Create a mock VDA 5050 v3.0.0 Order
    let sample_order = Order {
        header: Header {
            header_id: 1,
            timestamp: "2026-07-22T12:00:00Z".to_string(),
            version: "3.0.0".to_string(),
            manufacturer: "ISSEM".to_string(),
            serial_number: "AMR_01".to_string(),
        },
        order_id: "ORD_1001".to_string(),
        order_update_id: 0,
        zone_set_id: None,
        nodes: vec![Node {
            node_id: "Station_A".to_string(),
            sequence_id: 0,
            node_description: Some("Pickup station".to_string()),
            released: true,
            node_position: Some(NodePosition {
                x: 10.5,
                y: 5.2,
                theta: Some(0.0),
                map_id: "Warehouse_Floor_1".to_string(),
                map_description: None,
            }),
            actions: vec![],
        }],
        edges: vec![],
    };

    let json_output = serde_json::to_string_pretty(&sample_order)?;
    println!("Generated VDA 5050 v3.0.0 JSON:\n{}", json_output);

    Ok(())
}