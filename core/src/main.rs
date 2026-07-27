use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast; // 🆕 NEW: Added for live WebSocket broadcasting
use tower_http::services::ServeDir; // 🆕 NEW: Added for serving frontend HTML/JS
use tracing::{error, info, warn};
use zenoh::config::Config;

// Import from your library crate
use issem_core::api::{self, AppState};
use issem_core::fleet::FleetManager;
use issem_core::router::TopologicalRouter;
use issem_core::vda5050::State as VdaState;
use issem_core::ws::ws_handler; // 🆕 NEW: Import your WebSocket handler

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting ISSEM FMS Core Engine...");

    // 🆕 NEW: Create the broadcast channel for real-time telemetry (100 message buffer)
    let (tx, _rx) = broadcast::channel::<String>(100);

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

    router.add_node("node_A1", 0.0, 0.0);
    router.add_node("node_A2", 5.0, 0.0);
    router.add_node("node_A3", 10.0, 0.0);

    router.add_edge("edge_A1_A2", "node_A1", "node_A2", 5.0).unwrap();
    router.add_edge("edge_A2_A3", "node_A2", "node_A3", 5.0).unwrap();

    let router = Arc::new(router);
    info!("Topological route graph initialized.");

    // 3. Open Eclipse Zenoh Session (wrapped in Arc for multi-task sharing)
    info!("Opening Zenoh network session...");
    let zenoh_config = Config::default();
    let zenoh_session = Arc::new(
        zenoh::open(zenoh_config)
            .await
            .map_err(|e| {
                error!("Failed to open Zenoh session: {e}");
                e
            })?
    );

    // 4. (Optional) Initial Test Dispatch
    info!("Triggering initial test order dispatch...");
    match fleet_manager
        .dispatch_order(
            &zenoh_session,
            &router,
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
            warn!("Initial test order dispatch skipped or failed: {err}");
        }
    }

    // 5. Declare Zenoh Subscriber for VDA 5050 state topics
    let topic_pattern = "issem/v3/*/*/state";
    info!("Declaring Zenoh subscriber on topic pattern: '{topic_pattern}'");

    let subscriber = zenoh_session
        .declare_subscriber(topic_pattern)
        .await
        .map_err(|e| {
            error!("Failed to declare subscriber: {e}");
            e
        })?;

    // 6. Spawn Telemetry Listener in a Background Task
    let fleet_mgr_telemetry = fleet_manager.clone();
    let tx_telemetry = tx.clone(); // 🆕 NEW: Clone channel sender for the background task

    tokio::spawn(async move {
        info!("ISSEM FMS Core active. Telemetry listener thread started.");

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
            let fm = fleet_mgr_telemetry.clone();
            let tx_inner = tx_telemetry.clone(); // 🆕 NEW: Pass sender to inner task

            tokio::spawn(async move {
                let raw_json = String::from_utf8_lossy(&payload).to_string();
                let _ = tx_inner.send(raw_json.clone());

                // 🆕 NEW: Instantly forward the raw JSON telemetry over WebSockets to UI clients!

                match serde_json::from_slice::<VdaState>(&payload) {
                    Ok(vda_state) => {
                        info!(
                            "✅ [DESERIALIZATION] Parsed state for {mfr}:{sn} (Current Node: '{}')",
                            vda_state.last_node_id
                        );

                        if let Err(err) = fm.update_robot_state(&mfr, &sn, &vda_state).await {
                            error!("❌ Failed to persist telemetry to Redis for {mfr}:{sn}: {err}");
                        }
                    }
                    Err(err) => {
                        warn!("⚠️ [DESERIALIZATION ERROR] Failed to parse payload from {mfr}:{sn}: {err}");
                        warn!("Raw JSON was: {raw_json}");
                    }
                }
            });
        }
    });

    // 7. Setup & Run Axum REST API Server (Main Thread)
    let shared_state = AppState {
        router,
        fleet_manager,
        zenoh_session: zenoh_session.clone(),
        tx: tx.clone(),
    };

    let app = axum::Router::new()
        .route("/api/v1/robots/{robot_id}/orders", axum::routing::post(api::dispatch_order_handler))
        .route("/api/v1/tasks", axum::routing::post(api::create_transport_task_handler))
        .route("/api/v1/ws", axum::routing::get(ws_handler))
        .fallback_service(ServeDir::new("static")) // 👈 Change .nest_service("/", ...) to .fallback_service(...)
        .with_state(shared_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("🚀 ISSEM Core API server listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}