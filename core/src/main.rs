use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;
use serde::Deserialize;
use std::collections::HashMap;
use tokio::sync::broadcast;
use tower_http::services::ServeDir;
use tracing::{error, info, warn};
use zenoh::config::Config;

use issem_core::api::{self, AppState};
use issem_core::config::AppConfig;
use issem_core::fleet::FleetManager;
use issem_core::router::TopologicalRouter;
use issem_core::vda5050::State as VdaState;
use issem_core::ws::ws_handler;

#[derive(Deserialize)]
struct NodePos {
    x: f64,
    y: f64,
}

#[derive(Deserialize)]
struct GraphConfig {
    nodes: HashMap<String, NodePos>,
    edges: Vec<(String, String, f64)>, // [from, to, distance]
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    info!("Starting ISSEM FMS Core Engine...");

    // 1. Load centralized configuration from config/default.json or env vars
    let config = AppConfig::load()?;

    let (tx, _rx) = broadcast::channel::<String>(100);

    info!("Connecting to Redis state store at: {}", config.redis.url);
    let fleet_manager = match FleetManager::new(&config.redis.url).await {
        Ok(fm) => {
            info!("Successfully connected to Redis.");
            fm
        }
        Err(err) => {
            error!("Failed to initialize Redis FleetManager: {err}");
            return Err(err.into());
        }
    };

    // 2. Initialize and build the Topological Map Graph dynamically from graph.json
    let mut router = TopologicalRouter::new();
    let graph_path = &config.paths.graph_file;

    match fs::read_to_string(graph_path) {
        Ok(content) => {
            let graph: GraphConfig = serde_json::from_str(&content)?;
            
            for (node_id, pos) in &graph.nodes {
                router.add_node(node_id, pos.x, pos.y);
            }

            for (from, to, dist) in &graph.edges {
                let edge_id = format!("edge_{}_{}", from, to);
                if let Err(e) = router.add_edge(&edge_id, from, to, *dist) {
                    warn!("Failed to add edge {edge_id}: {e}");
                }
            }
            info!("Loaded {} nodes and {} edges from {}", graph.nodes.len(), graph.edges.len(), graph_path);
        }
        Err(e) => {
            warn!("Could not read {graph_path}: {e}. Initializing empty router.");
        }
    }

    let router = Arc::new(router);

    // 3. Open Eclipse Zenoh Session
    info!("Opening Zenoh network session...");
    let mut zenoh_config = Config::default();

    let listen_json = format!(r#"["{}"]"#, config.zenoh.listen_endpoint);
    
    // Using the literal string path "listen/endpoints" instead of the missing constant
    zenoh_config
        .insert_json5("listen/endpoints", &listen_json)
        .map_err(|e| {
            error!("Failed to set Zenoh listen endpoint: {e}");
            e
        })?;


    let zenoh_session = Arc::new(
        zenoh::open(zenoh_config)
            .await
            .map_err(|e| {
                error!("Failed to open Zenoh session: {e}");
                e
            })?
    );

    // 4. Declare Zenoh Subscriber using topic pattern from config
    let topic_pattern = &config.zenoh.state_topic_pattern;
    info!("Declaring Zenoh subscriber on topic pattern: '{topic_pattern}'");

    let subscriber = zenoh_session
        .declare_subscriber(topic_pattern)
        .await
        .map_err(|e| {
            error!("Failed to declare subscriber: {e}");
            e
        })?;

    // 5. Spawn Telemetry Listener Background Task
    let fleet_mgr_telemetry = fleet_manager.clone();
    let tx_telemetry = tx.clone();

    tokio::spawn(async move {
        info!("ISSEM FMS Core active. Telemetry listener thread started.");

        while let Ok(sample) = subscriber.recv_async().await {
            let key_expr = sample.key_expr().to_string();
            let payload = sample.payload().to_bytes().to_vec();

            let parts: Vec<&str> = key_expr.split('/').collect();
            if parts.len() != 5 {
                continue;
            }

            let mfr = parts[2].to_string();
            let sn = parts[3].to_string();
            let fm = fleet_mgr_telemetry.clone();
            let tx_inner = tx_telemetry.clone();

            tokio::spawn(async move {
                let raw_json = String::from_utf8_lossy(&payload).to_string();
                let _ = tx_inner.send(raw_json.clone());

                if let Ok(vda_state) = serde_json::from_slice::<VdaState>(&payload) {
                    let _ = fm.update_robot_state(&mfr, &sn, &vda_state).await;
                }
            });
        }
    });

    // 6. Setup & Run Axum REST API Server
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
        .fallback_service(ServeDir::new("static"))
        .with_state(shared_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
    info!("🚀 ISSEM Core API server listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}