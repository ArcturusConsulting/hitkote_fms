use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};

use crate::api::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryPayload {
    pub event: String,
    pub robot_id: String,
    pub state: String,
    pub battery: f32,
    pub last_node_id: String,
    pub position: Position2D,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position2D {
    pub x: f64,
    pub y: f64,
    pub theta: f64,
}

// Axum WebSocket Upgrade Handler
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handles an active WebSocket connection
async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.tx.subscribe();

    // Task 1: Forward broadcasted telemetry messages
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Task 2: Listen for incoming client close/ping events
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Close(_) = msg {
                break;
            }
        }
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    };
}