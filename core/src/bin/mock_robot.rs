use issem_core::fleet::fleet_manager::FleetManager; // Adjust path if re-exported differently in lib.rs
use issem_core::vda5050::{BatteryState, Header, OperatingMode, State as VdaState};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use zenoh::config::Config;

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

    // 1. Initialize FleetManager to manage Redis spatial locks
    let redis_url = "redis://127.0.0.1:6379";
    info!("Connecting to Redis at {redis_url}...");
    let fleet_mgr = FleetManager::new(redis_url).await?;

    // 2. Open Zenoh session
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

    // Defined waypoints for testing reactive lock releases
    let route = vec!["station_alpha", "node_A1", "node_A2", "node_A3", "node_A4"];
    let mut route_idx = 0;

    loop {
        // At the start of every lap, reserve all path nodes in Redis (60-second lease)
        if route_idx == 0 {
            info!("🔒 [LAP START] Pre-reserving path in Redis: {:?}", route);
            match fleet_mgr.try_reserve_path(&route, sn, 60).await {
                Ok(true) => info!("✅ Path reserved successfully in Redis for {sn}"),
                Ok(false) => tracing::warn!("⚠️ Could not reserve path (one or more nodes locked)"),
                Err(e) => tracing::error!("❌ Error reserving path: {e}"),
            }
        }

        sequence_id += 1;
        let current_node = route[route_idx];

        // Construct canonical VDA 5050 State struct
        let payload = VdaState {
            header: Header {
                header_id: sequence_id,
                timestamp: chrono::Utc::now().to_rfc3339(),
                version: "3.0.0".to_string(),
                manufacturer: mfr.to_string(),
                serial_number: sn.to_string(),
            },
            order_id: "ORD-2026-101".to_string(),
            order_update_id: 1,
            last_node_id: current_node.to_string(),
            last_node_sequence_id: (route_idx * 2) as u32,
            node_states: vec![],
            edge_states: vec![],
            driving: true,
            paused: Some(false),
            battery_state: BatteryState {
                battery_charge: 92.4,
                charging: false,
                battery_voltage: None,
            },
            operating_mode: OperatingMode::Automatic,
            action_states: vec![],
            errors: vec![],
        };

        let json_bytes = serde_json::to_vec(&payload)?;
        publisher.put(json_bytes).await.map_err(|e| {
            tracing::error!("Failed to publish VDA 5050 payload: {e}");
            e
        })?;

        info!("Published telemetry frame seq={sequence_id}, node='{current_node}' to '{topic}'");

        route_idx = (route_idx + 1) % route.len();
        sleep(Duration::from_secs(3)).await;
    }
}