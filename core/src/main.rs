mod vda5050;

use std::error::Error;
use tokio::time::{sleep, Duration};
use vda5050::{Header, Node, NodePosition, Order, State};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("🚀 ISSEM FMS Core Initializing...");

    // 1. Open Zenoh Session
    println!("📡 Opening Zenoh session...");
    let session = zenoh::open(zenoh::Config::default())
        .await
        .map_err(|e| format!("Failed to open Zenoh session: {e}"))?;
    println!("✅ Zenoh session active!");

    // 2. Subscribe to incoming State updates from ALL robots
    // Wildcard '*' matches any manufacturer and serial number
    let state_topic = "vda5050/v3/*/*/state";
    let subscriber = session
        .declare_subscriber(state_topic)
        .await
        .map_err(|e| format!("Failed to declare subscriber: {e}"))?;

    println!("📥 Subscribed to robot state topic: {}", state_topic);

    // Spawn an asynchronous background task to process incoming robot messages
    tokio::spawn(async move {
        while let Ok(sample) = subscriber.recv_async().await {
            // Safely attempt UTF-8 conversion using try_to_string()
            if let Ok(payload_str) = sample.payload().try_to_string() {
                match serde_json::from_str::<State>(&payload_str) {
                    Ok(state) => {
                        println!(
                            "🤖 [ROBOT STATE] ID: {} | Battery: {:.1}% | Mode: {:?} | Driving: {}",
                            state.header.serial_number,
                            state.battery_state.battery_charge,
                            state.operating_mode,
                            state.driving
                        );
                    }
                    Err(_) => {
                        println!(
                            "📩 [RAW MSG] Topic: {} | Payload: {}",
                            sample.key_expr(),
                            payload_str
                        );
                    }
                }
            }
        }
    });

    // 3. Declare Publisher for outbound Orders
    let order_topic = "vda5050/v3/ISSEM/AMR_01/order";
    let publisher = session
        .declare_publisher(order_topic)
        .await
        .map_err(|e| format!("Failed to declare publisher: {e}"))?;

    println!("📤 Order publisher bound to topic: {}", order_topic);

    // Give Zenoh discovery a moment to discover local peers
    sleep(Duration::from_millis(500)).await;

    // 4. Construct & publish a test VDA 5050 Order payload
    let test_order = Order {
        header: Header {
            header_id: 1,
            timestamp: "2026-07-22T13:00:00Z".to_string(),
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

    let json_payload = serde_json::to_string(&test_order)?;
    publisher.put(json_payload).await.map_err(|e| format!("Publish failed: {e}"))?;

    println!("✨ Published VDA 5050 Order to Zenoh topic [{}]", order_topic);
    println!("⏳ Core running. Listening for robot states... (Press Ctrl+C to exit)");

    // Keep event loop alive
    tokio::signal::ctrl_c().await?;
    println!("\n🛑 Shutting down ISSEM Core.");

    Ok(())
}