use std::error::Error;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("🤖 Starting Mock Robot (AMR_01)...");

    let session = zenoh::open(zenoh::Config::default())
        .await
        .map_err(|e| format!("Failed to open Zenoh session: {e}"))?;

    let key_expr = "vda5050/v3/ISSEM/AMR_01/state";
    let publisher = session
        .declare_publisher(key_expr)
        .await
        .map_err(|e| format!("Failed to declare publisher: {e}"))?;

    // Wait briefly for discovery
    sleep(Duration::from_millis(500)).await;

    let mock_state_json = serde_json::json!({
        "header": {
            "headerId": 10,
            "timestamp": "2026-07-22T13:00:00Z",
            "version": "3.0.0",
            "manufacturer": "ISSEM",
            "serialNumber": "AMR_01"
        },
        "orderId": "ORD_1001",
        "orderUpdateId": 0,
        "lastNodeId": "Station_A",
        "lastNodeSequenceId": 0,
        "nodeStates": [],
        "edgeStates": [],
        "driving": false,
        "batteryState": {
            "batteryCharge": 88.5,
            "charging": false
        },
        "operatingMode": "AUTOMATIC"
    }).to_string();

    publisher.put(mock_state_json).await.map_err(|e| format!("Publish failed: {e}"))?;
    println!("📡 Mock State published to topic [{}]", key_expr);

    // Keep session alive briefly to ensure transmission
    sleep(Duration::from_millis(500)).await;
    Ok(())
}