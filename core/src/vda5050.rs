#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Standard VDA 5050 Header attached to every message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    pub header_id: u32,
    pub timestamp: String, // ISO 8601 UTC timestamp
    pub version: String,   // e.g., "3.0.0"
    pub manufacturer: String,
    pub serial_number: String,
}

// ============================================================================
// 1. ORDER INTERFACE (FMS -> Robot)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub header: Header,
    pub order_id: String,
    pub order_update_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone_set_id: Option<String>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub node_id: String,
    pub sequence_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_description: Option<String>,
    pub released: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_position: Option<NodePosition>,
    #[serde(default)]
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NodePosition {
    pub x: f64,
    pub y: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theta: Option<f64>,
    pub map_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub map_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub edge_id: String,
    pub sequence_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_description: Option<String>,
    pub released: bool,
    pub start_node_id: String,
    pub end_node_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_speed: Option<f64>,
    #[serde(default)]
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub action_type: String, // e.g., "pick", "drop", "charge", "pause"
    pub action_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_description: Option<String>,
    pub blocking_type: BlockingType,
    #[serde(default)]
    pub action_parameters: Vec<ActionParameter>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BlockingType {
    None,
    Soft,
    Hard,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActionParameter {
    pub key: String,
    pub value: serde_json::Value,
}

// ============================================================================
// 2. INSTANT ACTIONS INTERFACE (FMS -> Robot)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstantActions {
    pub header: Header,
    pub actions: Vec<Action>,
}

// ============================================================================
// 3. STATE INTERFACE (Robot -> FMS)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub header: Header,
    pub order_id: String,
    pub order_update_id: u32,
    pub last_node_id: String,
    pub last_node_sequence_id: u32,
    #[serde(default)]
    pub node_states: Vec<NodeState>,
    #[serde(default)]
    pub edge_states: Vec<EdgeState>,
    pub driving: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paused: Option<bool>,
    pub battery_state: BatteryState,
    pub operating_mode: OperatingMode,
    #[serde(default)]
    pub action_states: Vec<ActionState>,
    #[serde(default)]
    pub errors: Vec<ErrorEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NodeState {
    pub node_id: String,
    pub sequence_id: u32,
    pub released: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EdgeState {
    pub edge_id: String,
    pub sequence_id: u32,
    pub released: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BatteryState {
    pub battery_charge: f64, // Percentage (0.0 to 100.0)
    pub charging: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battery_voltage: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OperatingMode {
    Automatic,
    SemiAutomatic,
    Manual,
    Service,
    Teaching,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActionState {
    pub action_id: String,
    pub action_status: ActionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActionStatus {
    Waiting,
    Initializing,
    Running,
    Finished,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEntry {
    pub error_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_description: Option<String>,
    pub error_level: ErrorLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorLevel {
    Warning,
    Fatal,
}

// ============================================================================
// TOPIC BUILDERS (VDA 5050 Wire Protocol Constants & Utilities)
// ============================================================================

impl Header {
    /// Generates a standard VDA 5050 topic string given a subtopic name.
    /// Pattern: `uagv/v2/{manufacturer}/{serial_number}/{subtopic}`
    pub fn topic(&self, subtopic: &str) -> String {
        format!(
            "uagv/v2/{}/{}/{}",
            self.manufacturer, self.serial_number, subtopic
        )
    }
}

impl Order {
    /// Returns the Zenoh/MQTT topic for this specific order instance.
    pub fn topic(&self) -> String {
        self.header.topic("order")
    }

    /// Constructs an order topic directly from manufacturer and serial number.
    pub fn topic_for(manufacturer: &str, serial_number: &str) -> String {
        format!("uagv/v2/{manufacturer}/{serial_number}/order")
    }
}

impl InstantActions {
    /// Returns the Zenoh/MQTT topic for this instant action instance.
    pub fn topic(&self) -> String {
        self.header.topic("instantActions")
    }

    /// Constructs an instantActions topic directly from manufacturer and serial number.
    pub fn topic_for(manufacturer: &str, serial_number: &str) -> String {
        format!("uagv/v2/{manufacturer}/{serial_number}/instantActions")
    }
}

impl State {
    /// Returns the Zenoh/MQTT topic for this state message instance.
    pub fn topic(&self) -> String {
        self.header.topic("state")
    }

    /// Constructs a state topic directly from manufacturer and serial number.
    pub fn topic_for(manufacturer: &str, serial_number: &str) -> String {
        format!("uagv/v2/{manufacturer}/{serial_number}/state")
    }

    /// Subscribing wildcard topic for all incoming AMR states across manufacturers.
    pub fn wildcard_topic() -> &'static str {
        "uagv/v2/*/*/state"
    }
}