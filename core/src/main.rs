mod fleet;
mod vda5050;

use fleet::FleetManager;
use vda5050::State as VdaState;
use std::env;
use tracing::{error, info, warn};
use zenoh::config::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting ISSEM FMS Core Engine...");

    // 1. Initialize Redis Connection Manager
    let redis_url = env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    info!("Connecting to Redis state store at: {redis_url}");

    let fleet_manager = match FleetManager::new(&redis_url).await {
        Ok(fm) => {
            info!("Successfully connected to Redis.");
            fm
        }
        Err(err) => {
            error!("Failed to initialize Redis FleetManager: {err}");
            return Err(err.into());
        }
    };

    // 2. Open Eclipse Zenoh Session
    info!("Opening Zenoh network session...");
    let zenoh_config = Config::default();
    let session = zenoh::open(zenoh_config).await.map_err(|e| {
        error!("Failed to open Zenoh session: {e}");
        e
    })?;

    // 3. Subscribe to VDA 5050 state topics: issem/v3/{manufacturer}/{serialNumber}/state
    let topic_pattern = "issem/v3/*/*/state";
    info!("Declaring Zenoh subscriber on topic pattern: '{topic_pattern}'");

    let subscriber = session
        .declare_subscriber(topic_pattern)
        .await
        .map_err(|e| {
            error!("Failed to declare subscriber: {e}");
            e
        })?;

    info!("ISSEM FMS Core active. Listening for telemetry...");

    // 4. Telemetry Processing Event Loop
    while let Ok(sample) = subscriber.recv_async().await {
        let key_expr = sample.key_expr().to_string();

        // Convert the borrowed slice into an owned Vec<u8> so it can cross the tokio::spawn boundary
        let payload = sample.payload().to_bytes().to_vec();

        // Extract {manufacturer} and {serialNumber} from "issem/v3/{mfr}/{sn}/state"
        let parts: Vec<&str> = key_expr.split('/').collect();
        if parts.len() != 5 {
            warn!("Received message on invalid key expression format: {key_expr}");
            continue;
        }

        let mfr = parts[2].to_string();
        let sn = parts[3].to_string();
        let fleet_mgr = fleet_manager.clone();

        // Spawn non-blocking background task per update
        tokio::spawn(async move {
            match serde_json::from_slice::<VdaState>(&payload) {
                Ok(vda_state) => {
                    if let Err(err) = fleet_mgr.update_robot_state(&mfr, &sn, &vda_state).await {
                        error!("Failed to persist telemetry to Redis for {mfr}:{sn}: {err}");
                    }
                }
                Err(err) => {
                    warn!("Failed to deserialize VDA 5050 JSON payload from {mfr}:{sn}: {err}");
                }
            }
        });
    }

    Ok(())
}