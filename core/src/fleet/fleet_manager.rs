use crate::vda5050::State as VdaState;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde_json;
use std::fmt;

/// Custom error type for fleet management operations
#[derive(Debug)]
pub enum FleetError {
    Redis(redis::RedisError),
    Serialization(serde_json::Error),
}

impl fmt::Display for FleetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FleetError::Redis(err) => write!(f, "Redis error: {err}"),
            FleetError::Serialization(err) => write!(f, "JSON serialization error: {err}"),
        }
    }
}

impl std::error::Error for FleetError {}

impl From<redis::RedisError> for FleetError {
    fn from(err: redis::RedisError) -> Self {
        FleetError::Redis(err)
    }
}

impl From<serde_json::Error> for FleetError {
    fn from(err: serde_json::Error) -> Self {
        FleetError::Serialization(err)
    }
}

/// Async Fleet Manager responsible for persisting telemetry snapshots,
/// tracking robot heartbeats, and managing spatial lease locks.
#[derive(Clone)]
pub struct FleetManager {
    redis: ConnectionManager,
}

impl FleetManager {
    /// Connects to Redis and establishes a resilient async connection manager.
    pub async fn new(redis_url: &str) -> Result<Self, FleetError> {
        let client = redis::Client::open(redis_url)?;
        let manager = ConnectionManager::new(client).await?;
        Ok(Self { redis: manager })
    }

    /// Stores the latest VDA 5050 state snapshot and refreshes the robot's liveness ping (5s TTL).
    pub async fn update_robot_state(
        &self,
        mfr: &str,
        sn: &str,
        state: &VdaState,
    ) -> Result<(), FleetError> {
        let state_json = serde_json::to_string(state)?;
        let state_key = format!("issem:robot:{mfr}:{sn}:state");
        let heartbeat_key = format!("issem:robot:{mfr}:{sn}:heartbeat");

        let mut conn = self.redis.clone();

        // 1. Persist the full telemetry snapshot
        let _: () = conn.set(&state_key, state_json).await?;

        // 2. Touch the heartbeat key with a 5-second TTL lease
        let _: () = conn.set_ex(&heartbeat_key, "ONLINE", 5).await?;

        Ok(())
    }

    /// Fetches the latest telemetry snapshot for a specific robot.
    pub async fn get_robot_state(
        &self,
        mfr: &str,
        sn: &str,
    ) -> Result<Option<VdaState>, FleetError> {
        let state_key = format!("issem:robot:{mfr}:{sn}:state");
        let mut conn = self.redis.clone();

        let raw_json: Option<String> = conn.get(&state_key).await?;
        match raw_json {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }

    /// Tries to acquire an exclusive spatial node lock using a leased TTL lock (`SET NX PX`).
    /// Returns `true` if acquired, `false` if already locked by another robot.
    pub async fn try_acquire_node_lock(
        &self,
        node_id: &str,
        robot_id: &str,
        ttl_ms: u64,
    ) -> Result<bool, FleetError> {
        let lock_key = format!("issem:traffic:node:{node_id}");
        let mut conn = self.redis.clone();

        // Performs: SET issem:traffic:node:{node_id} {robot_id} NX PX {ttl_ms}
        let result: Option<String> = redis::cmd("SET")
            .arg(&lock_key)
            .arg(robot_id)
            .arg("NX")
            .arg("PX")
            .arg(ttl_ms)
            .query_async(&mut conn)
            .await?;

        Ok(result.is_some())
    }

    /// Releases a node lock if and only if it is still owned by the given robot.
    pub async fn release_node_lock(&self, node_id: &str, robot_id: &str) -> Result<bool, FleetError> {
        let lock_key = format!("issem:traffic:node:{node_id}");
        let mut conn = self.redis.clone();

        // Atomic evaluation using a simple inline Lua script to prevent accidental deletion
        // if the lock expired and was re-acquired by another AMR.
        let script = redis::Script::new(
            r#"
            if redis.call("get", KEYS[1]) == ARGV[1] then
                return redis.call("del", KEYS[1])
            else
                return 0
            end
            "#,
        );

        let released: i32 = script
            .key(&lock_key)
            .arg(robot_id)
            .invoke_async(&mut conn)
            .await?;

        Ok(released == 1)
    }
}