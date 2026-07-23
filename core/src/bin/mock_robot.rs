use serde::Serialize;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use zenoh::config::Config;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Header {
    header_id: u32,
    timestamp: String,
    version: String,
    manufacturer: String,
    serial_number: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BatteryState {
    battery_charge: f64,
    charging: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SafetyState {
    e_stop: bool,
    field_violation: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VdaStatePayload {
    header: Header,
    order_id: String,
    order_update_id: u32,
    last_node_id: String,
    last_node_sequence_id: u32,
    driving: bool,
    operating_mode: String,
    battery_state: BatteryState,
    safety_state: SafetyState,
    node_states: Vec<serde_json::Value>,
    edge_states: Vec<serde_json::Value>,
    action_states: Vec<serde_json::Value>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting Mock AMR Publisher [ISSEM / AMR-01]...");

    let mfr = "ISSEM";
    let sn = "AMR-01";
    let topic = format!("issem/v3/{mfr}/{sn}/state");

    info!("Opening Zenoh session...");
    let session = zenoh::open(Config::default()).await.map_err(|e| {
        tracing::error!("Failed to open Zenoh session: {e}");
        e
    })?;

    info!("Declaring Zenoh publisher on topic: '{topic}'");
    let publisher = session.declare_publisher(&topic).await.map_err(|e| {
        tracing::error!("Failed to declare publisher: {e}");
        e
    })?;

    let mut sequence_id: u32 = 0;

    loop {
        sequence_id += 1;

        let payload = VdaStatePayload {
            header: Header {
                header_id: sequence_id,
                timestamp: chrono::Utc::now().to_rfc3339(),
                version: "3.0.0".to_string(),
                manufacturer: mfr.to_string(),
                serial_number: sn.to_string(),
            },
            order_id: "ORD-2026-101".to_string(),
            order_update_id: 1,
            last_node_id: "station_alpha".to_string(),
            last_node_sequence_id: 2,
            driving: true,
            operating_mode: "AUTOMATIC".to_string(),
            battery_state: BatteryState {
                battery_charge: 92.4,
                charging: false,
            },
            safety_state: SafetyState {
                e_stop: false,
                field_violation: false,
            },
            node_states: vec![],
            edge_states: vec![],
            action_states: vec![],
        };

        let json_bytes = serde_json::to_vec(&payload)?;
        publisher.put(json_bytes).await.map_err(|e| {
            tracing::error!("Failed to publish VDA 5050 payload: {e}");
            e
        })?;

        info!("Published telemetry frame seq={sequence_id} to '{topic}'");

        sleep(Duration::from_secs(1)).await;
    }
}