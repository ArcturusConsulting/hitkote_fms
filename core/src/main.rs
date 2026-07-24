use std::env;
use std::sync::Arc;
use tracing::{error, info, warn};
use zenoh::config::Config;

use issem_core::fleet::FleetManager;
use issem_core::router::{MapEdge, MapNode, TopologicalRouter};
use issem_core::vda5050::State as VdaState;

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

// 2. Build the Topological Map Graph
    let mut router = TopologicalRouter::new();

    // Pass (id, x, y) as floats directly:
    router.add_node("node_A1", 0.0, 0.0);
    router.add_node("node_A2", 5.0, 0.0);
    router.add_node("node_A3", 10.0, 0.0);

    // Pass (edge_id, from_id, to_id, distance_m) directly:
    router.add_edge("edge_A1_A2", "node_A1", "node_A2", 5.0).unwrap();
    router.add_edge("edge_A2_A3", "node_A2", "node_A3", 5.0).unwrap();

    // Wrap router in Arc so it can be shared safely across tasks if needed
    let router = Arc::new(router);
    info!("Topological route graph initialized.");

    // 3. Open Eclipse Zenoh Session
    info!("Opening Zenoh network session...");
    let zenoh_config = Config::default();
    let session = zenoh::open(zenoh_config).await.map_err(|e| {
        error!("Failed to open Zenoh session: {e}");
        e
    })?;

    // 4. Test Dispatch an Order (Optional startup dispatch)
    let fm_dispatch = fleet_manager.clone();
    let router_dispatch = router.clone();
    let session_ref = &session;

    info!("Triggering initial test order dispatch...");
    match fm_dispatch
        .dispatch_order(
            session_ref,
            &router_dispatch,
            "Mir",
            "AMR-01",
            "node_A1",
            "node_A3",
            "ORD-2026-0001",
            60,
        )
        .await
    {
        Ok(order) => {
            info!(
                "Successfully dispatched order '{}' for Mir:AMR-01 across {} nodes!",
                order.order_id,
                order.nodes.len()
            );
        }
        Err(err) => {
            error!("Failed to dispatch order: {err}");
        }
    }

    // 5. Subscribe to VDA 5050 state topics: issem/v3/{manufacturer}/{serialNumber}/state
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

    // 6. Telemetry Processing Event Loop
    while let Ok(sample) = subscriber.recv_async().await {
        let key_expr = sample.key_expr().to_string();
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
                    info!(
                        "✅ [DESERIALIZATION] Parsed state for {mfr}:{sn} (Current Node: '{}')",
                        vda_state.last_node_id
                    );

                    if let Err(err) = fleet_mgr.update_robot_state(&mfr, &sn, &vda_state).await {
                        error!("❌ Failed to persist telemetry to Redis for {mfr}:{sn}: {err}");
                    }
                }
                Err(err) => {
                    warn!("⚠️ [DESERIALIZATION ERROR] Failed to parse payload from {mfr}:{sn}: {err}");
                    let raw_json = String::from_utf8_lossy(&payload);
                    warn!("Raw JSON was: {raw_json}");
                }
            }
        });
    }

    Ok(())
}