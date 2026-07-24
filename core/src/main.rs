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

    let fleet_test = fleet_manager.clone();
    tokio::spawn(async move {
        let path = vec!["node_A1", "node_A2", "node_A3"];
        let robot_1 = "AMR-01";
        let robot_2 = "AMR-02";

        tracing::info!("--- ATOMIC PATH RESERVATION & RELEASE TEST ---");

        // 1. AMR-01 reserves [A1, A2, A3]
        let path_1_ok = fleet_test
            .try_reserve_path(&path, robot_1, 15)
            .await
            .unwrap_or(false);

        if path_1_ok {
            tracing::info!("SUCCESS: [{}] reserved path {:?}", robot_1, path);
        }

        // 2. AMR-02 tries to steal path (blocked)
        let path_2_ok = fleet_test
            .try_reserve_path(&path, robot_2, 15)
            .await
            .unwrap_or(false);

        if !path_2_ok {
            tracing::info!("SUCCESS (Blocked): [{}] blocked from path reserved by {}", robot_2, robot_1);
        }

        // 3. AMR-01 releases the path
        let released = fleet_test
            .release_path(&path, robot_1)
            .await
            .unwrap_or(0);

        tracing::info!("SUCCESS: [{}] released {} node locks", robot_1, released);

        // 4. AMR-02 can now reserve the path
        let path_2_retry = fleet_test
            .try_reserve_path(&path, robot_2, 15)
            .await
            .unwrap_or(false);

        if path_2_retry {
            tracing::info!("SUCCESS: [{}] successfully acquired path after release!", robot_2);
        }

        // Cleanup
        let _ = fleet_test.release_path(&path, robot_2).await;
        tracing::info!("--- TEST COMPLETE ---");
    });

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
        
        // 🚨 ADDED: Prove Zenoh is receiving the message over the network
        info!("📩 [ZENOH] Received raw message on topic: {key_expr}");

        let payload = sample.payload().to_bytes().to_vec();

        let parts: Vec<&str> = key_expr.split('/').collect();
        if parts.len() != 5 {
            warn!("⚠️ Received message on invalid key expression format: {key_expr}");
            continue;
        }

        let mfr = parts[2].to_string();
        let sn = parts[3].to_string();
        let fleet_mgr = fleet_manager.clone();

        tokio::spawn(async move {
            match serde_json::from_slice::<VdaState>(&payload) {
                Ok(vda_state) => {
                    // 🚨 ADDED: Prove Serde parsed the VDA struct successfully
                    info!("✅ [DESERIALIZATION] Parsed state for {mfr}:{sn} (Current Node: '{}')", vda_state.last_node_id);
                    
                    if let Err(err) = fleet_mgr.update_robot_state(&mfr, &sn, &vda_state).await {
                        error!("❌ Failed to persist telemetry to Redis for {mfr}:{sn}: {err}");
                    }
                }
                Err(err) => {
                    warn!("⚠️ [DESERIALIZATION ERROR] Failed to parse payload from {mfr}:{sn}: {err}");
                    
                    // String::from_utf8_lossy takes &[u8] by reference and never panics or returns Err
                    let raw_json = String::from_utf8_lossy(&payload);
                    warn!("Raw JSON was: {raw_json}");
                }
            }
        });
    }

    Ok(())
}