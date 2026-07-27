use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::fleet::{FleetError, FleetManager, TaskAllocator, TransportTaskRequest};
use crate::router::TopologicalRouter;

/// Shared application state injected into Axum route handlers.
#[derive(Clone)]
pub struct AppState {
    pub router: Arc<TopologicalRouter>,
    pub fleet_manager: FleetManager,
    pub zenoh_session: Arc<zenoh::Session>,
    pub tx: tokio::sync::broadcast::Sender<String>,
}

/// Incoming JSON payload for order dispatching.
#[derive(Debug, Deserialize)]
pub struct DispatchRequest {
    pub robot_id: String,
    pub manufacturer: String,
    pub start_node: String,
    pub target_node: String,
    pub lock_ttl_secs: Option<u64>,
}

/// Outgoing HTTP response payload.
#[derive(Debug, Serialize)]
pub struct DispatchResponse {
    pub success: bool,
    pub order_id: String,
    pub path: Vec<String>,
    pub message: String,
}

/// POST /api/v1/orders/dispatch
/// POST /api/v1/tasks
/// Accepts high-level transport requests from WES/WCS and allocates the optimal robot.
pub async fn create_transport_task_handler(
    State(state): State<AppState>,
    Json(payload): Json<TransportTaskRequest>,
) -> impl IntoResponse {
    let mut redis_conn = state.fleet_manager.get_redis_conn();

    match TaskAllocator::allocate_and_dispatch(
        &mut redis_conn,
        &state.fleet_manager,
        &state.zenoh_session,
        &state.router,
        payload,
    )
    .await
    {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(err_msg) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": err_msg })),
        )
            .into_response(),
    }
}

pub async fn dispatch_order_handler(
    State(state): State<AppState>,
    Json(payload): Json<DispatchRequest>,
) -> impl IntoResponse {
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
    let short_hash = &uuid::Uuid::new_v4().simple().to_string()[..4];
    let order_id = format!("ORD-{timestamp}-{}-{short_hash}", payload.robot_id);
    let lock_ttl = payload.lock_ttl_secs.unwrap_or(60);

    // Delegate the entire routing, lock reservation, and Zenoh dispatch to FleetManager
    match state
        .fleet_manager
        .dispatch_order(
            &state.zenoh_session,
            &state.router,
            &payload.manufacturer,
            &payload.robot_id,
            &payload.start_node,
            &payload.target_node,
            &order_id,
            lock_ttl,
        )
        .await
    {
        Ok(vda_order) => {
            // Extract the path node IDs from the resulting VDA 5050 order
            let path: Vec<String> = vda_order.nodes.iter().map(|n| n.node_id.clone()).collect();

            (
                StatusCode::ACCEPTED,
                Json(DispatchResponse {
                    success: true,
                    order_id,
                    path,
                    message: "Order successfully reserved and dispatched".into(),
                }),
            )
        }
        Err(FleetError::NoRouteFound(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(DispatchResponse {
                success: false,
                order_id: String::new(),
                path: vec![],
                message: msg,
            }),
        ),
        Err(FleetError::PathOccupied(msg)) => (
            StatusCode::CONFLICT,
            Json(DispatchResponse {
                success: false,
                order_id: String::new(),
                path: vec![],
                message: msg,
            }),
        ),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(DispatchResponse {
                success: false,
                order_id: String::new(),
                path: vec![],
                message: err.to_string(),
            }),
        ),
    }
}