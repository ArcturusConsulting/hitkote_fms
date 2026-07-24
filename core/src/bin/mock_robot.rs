use issem_core::vda5050::{BatteryState, Header, OperatingMode, State as VdaState};
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;
use zenoh::config::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let mfr = "Mir";
    let sn = "AMR-01";
    info!("Starting Mock AMR Listener/Publisher [{mfr}:{sn}]...");

    // 1. Open Zenoh session
    info!("Opening Zenoh session...");
    let session = zenoh::open(Config::default()).await.map_err(|e| {
        error!("Failed to open Zenoh session: {e}");
        e
    })?;

    // Topics following VDA 5050 / ISSEM conventions
    let order_topic = format!("uagv/v2/{mfr}/{sn}/order");
    let state_topic = format!("issem/v3/{mfr}/{sn}/state");

    // 2. Subscribe to Orders sent by FMS Core (main.rs)
    info!("Declaring Zenoh subscriber on order topic: '{order_topic}'");
    let order_sub = session.declare_subscriber(&order_topic).await.map_err(|e| {
        error!("Failed to declare subscriber: {e}");
        e
    })?;

    // 3. Declare Publisher for State Telemetry
    info!("Declaring Zenoh publisher on state topic: '{state_topic}'");
    let publisher = session.declare_publisher(&state_topic).await.map_err(|e| {
        error!("Failed to declare publisher: {e}");
        e
    })?;

    info!("Mock Robot active. Waiting for order dispatches...");

    let mut sequence_id: u32 = 0;

    // 4. Main loop: Wait for Order, then simulate driving
    while let Ok(sample) = order_sub.recv_async().await {
        let payload = sample.payload().to_bytes();
        info!("📩 Received Order payload on {order_topic}");

        if let Ok(order_json) = serde_json::from_slice::<Value>(&payload) {
            let order_id = order_json
                .get("orderId")
                .and_then(|v| v.as_str())
                .unwrap_or("ORD-UNKNOWN");

            if let Some(nodes) = order_json.get("nodes").and_then(|n| n.as_array()) {
                info!("Executing Order '{order_id}' across {} nodes...", nodes.len());

                for (idx, node) in nodes.iter().enumerate() {
                    let node_id = node
                        .get("nodeId")
                        .and_then(|n| n.as_str())
                        .unwrap_or("unknown_node");

                    info!("🚗 Driving to '{node_id}'...");
                    sleep(Duration::from_secs(3)).await;

                    sequence_id += 1;

                    // Build canonical VDA 5050 State struct matching your crate
                    let state_payload = VdaState {
                        header: Header {
                            header_id: sequence_id,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            version: "3.0.0".to_string(),
                            manufacturer: mfr.to_string(),
                            serial_number: sn.to_string(),
                        },
                        order_id: order_id.to_string(),
                        order_update_id: 1,
                        last_node_id: node_id.to_string(),
                        last_node_sequence_id: (idx * 2) as u32,
                        node_states: vec![],
                        edge_states: vec![],
                        driving: true,
                        paused: Some(false),
                        battery_state: BatteryState {
                            battery_charge: 95.0,
                            charging: false,
                            battery_voltage: None,
                        },
                        operating_mode: OperatingMode::Automatic,
                        action_states: vec![],
                        errors: vec![],
                    };

                    let json_bytes = serde_json::to_vec(&state_payload)?;
                    publisher.put(json_bytes).await.map_err(|e| {
                        error!("Failed to publish state update: {e}");
                        e
                    })?;

                    info!("📍 Arrived at '{node_id}'. Telemetry published (seq={sequence_id}).");
                }

                info!("✅ Order '{order_id}' execution complete!");
            }
        } else {
            warn!("⚠️ Failed to parse incoming Order JSON payload.");
        }
    }

    Ok(())
}