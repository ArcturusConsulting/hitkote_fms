use std::net::SocketAddr;
use std::sync::Arc;
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> { // Returns nothing if everything succeeds; catch and return any type of error if something fails

    // 0. Initialize structured logging
    tracing_subscriber::fmt::init();
    info!("Starting ISSEM FMS Core Engine...");

    // 1. Load centralized configuration from config/default.json or env vars
    let config = AppConfig::load()?;

    // 2. creates a 1-to-many communication bridge (robot to redis & dashboard)
    let (tx, _rx) = broadcast::channel::<String>(100);
    info!("Connecting to Redis state store at: {}", config.redis.url);

    // 3. Initialize FleetManager with Redis connection
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

    // 4. Initialize the Topological Map Graph and let multiple threads access it concurrently
    let graph_path = &config.paths.graph_file;

    let router = match TopologicalRouter::from_file(graph_path) {
        Ok(r) => {
            info!("Successfully loaded topological graph from {graph_path}");
            r
        }
        Err(e) => {
            warn!("Could not read {graph_path}: {e}. Initializing empty router.");
            TopologicalRouter::new()
        }
    };

    let router = Arc::new(router);

    // 5. Open Eclipse Zenoh Session
    info!("Opening Zenoh network session...");
    let mut zenoh_config = Config::default();

    let listen_json = format!(r#"["{}"]"#, config.zenoh.listen_endpoint);
    
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

    // 6. Declare Zenoh Subscriber using topic pattern from config
    let topic_pattern = &config.zenoh.state_topic_pattern;
    info!("Declaring Zenoh subscriber on topic pattern: '{topic_pattern}'");

    let subscriber = zenoh_session
        .declare_subscriber(topic_pattern)
        .await
        .map_err(|e| {
            error!("Failed to declare subscriber: {e}");
            e
        })?;

    // Clone fleet manager and tx for use in telemetry listener task below [Outer Clone]
    let fleet_mgr_telemetry = fleet_manager.clone();

    let tx_telemetry = tx.clone();
    
    // 7. Spawn Telemetry Listener Background Task
    tokio::spawn(async move {
        info!("ISSEM FMS Core active. Telemetry listener thread started.");

        // Pauses until Zenoh receives a message
        while let Ok(sample) = subscriber.recv_async().await {
            // As soon as a message arrives, returns topic name and bytes 
            let key_expr = sample.key_expr().to_string();
            let payload = sample.payload().to_bytes().to_vec();
            
            // Key expressions have 5 segments split by / in VDA 5050 over MQTT/Zenoh
            let parts: Vec<&str> = key_expr.split('/').collect();
            if parts.len() != 5 {
                continue;
            }

            // Extracts manufacturer and serial number to identify the robot reporting its state
            let mfr = parts[2].to_string();
            let sn = parts[3].to_string();

            // Clone the fleet manager and tx for use in the async task below [Inner Clone]
            let fm = fleet_mgr_telemetry.clone();
            let tx_inner = tx_telemetry.clone();

            // Offloads the writing into a separate task to prevent lags
            tokio::spawn(async move {

                // Converts to string and broadcasts it directly to the dashboard via websocket
                let raw_json = String::from_utf8_lossy(&payload).to_string();
                let _ = tx_inner.send(raw_json.clone());

                // Deserialize the payload into VDA 5050 state and update the robot state in Redis
                if let Ok(vda_state) = serde_json::from_slice::<VdaState>(&payload) {
                    let _ = fm.update_robot_state(&mfr, &sn, &vda_state).await;
                }
            });
        }
    });

    // 8. Setup an Axum REST API Server for integration with external systems, e.g., WES
    let shared_state = AppState {
        router,
        fleet_manager,
        zenoh_session: zenoh_session.clone(),
        tx: tx.clone(),
    };

    // 9. Instantiates a fresh Axum HTTP router.
    let app = axum::Router::new()
        // Allows external systems to send a specific order directly to a designated robot
        .route("/api/v1/robots/{robot_id}/orders", axum::routing::post(api::dispatch_order_handler))

        // Transfer the request; FleetManager will find an available AMR, uses TopologicalRouter to compute the path, and dispatches the task automatically
        .route("/api/v1/tasks", axum::routing::post(api::create_transport_task_handler))
        
        // Streams live VDA 5050 telemetry (robot positions, battery, errors) directly to the dashboard with minimal latency.
        .route("/api/v1/ws", axum::routing::get(ws_handler))

        // Serves static files (HTML, JS, CSS) for the dashboard
        .fallback_service(ServeDir::new("static"))

        // Uses the shared state defined above
        .with_state(shared_state);

    // 10. Start the Axum HTTP server and listen for incoming requests
    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
    info!("ISSEM Core API server listening on http://{addr}");

    // 11. Binds the Axum server to the specified address and port, and starts serving requests
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}