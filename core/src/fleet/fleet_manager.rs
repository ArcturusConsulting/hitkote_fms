use crate::router::TopologicalRouter;
use crate::vda5050::{Order, State as VdaState};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use zenoh::Session;

/// Custom error type for fleet management operations
#[derive(Debug, thiserror::Error)]
pub enum FleetError {
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("JSON serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Zenoh transport error: {0}")]
    Zenoh(#[from] zenoh::Error),

    #[error("Pathfinding error: {0}")]
    NoRouteFound(String),

    #[error("Lock conflict: {0}")]
    PathOccupied(String),
}

/// Async Fleet Manager responsible for persisting telemetry snapshots,
/// tracking robot heartbeats, and managing spatial lease locks.
#[derive(Clone)]
pub struct FleetManager {
    redis: ConnectionManager,
}

impl FleetManager {
    // Creates a new lightweight handle to the Redis connection manager
    pub fn get_redis_conn(&self) -> redis::aio::ConnectionManager {
            self.redis.clone()
        }

    // Connects to Redis and establishes a resilient async connection manager
    pub async fn new(redis_url: &str) -> Result<Self, FleetError> {
        let client = redis::Client::open(redis_url)?;
        let manager = ConnectionManager::new(client).await?;
        Ok(Self { redis: manager })
    }

    // Dispatches a VDA 5050 order to a specific robot 
    pub async fn dispatch_order(
        &self,
        zenoh: &Session,
        router: &TopologicalRouter,
        manufacturer: &str,
        serial_number: &str,
        start_node: &str,
        target_node: &str,
        order_id: &str,
        lock_ttl_secs: u64,
    ) -> Result<Order, FleetError> {
        // 1. Calculate path using the topological router
        let plan = router.find_path(start_node, target_node).ok_or_else(|| {
            FleetError::NoRouteFound(format!("No path found from '{start_node}' to '{target_node}'"))
        })?;

        // 2. Atomically reserve all nodes on the path (make sure the path is not locked by another robot)
        let node_refs: Vec<&str> = plan.node_ids.iter().map(|s| s.as_str()).collect();
        let acquired = self
            .try_reserve_path(&node_refs, serial_number, lock_ttl_secs)
            .await?;

        if !acquired {
            return Err(FleetError::PathOccupied(format!(
                "Cannot dispatch order '{order_id}': one or more nodes on path are locked by another AMR"
            )));
        }

        // 3. Convert RoutePlan into a valid VDA 5050 Order
        let order = plan.into_vda5050_order(order_id, 0, manufacturer, serial_number, router);

        // 4. Serialize and publish over Zenoh
        let topic = order.topic();
        let payload = serde_json::to_string(&order)?;

        zenoh.put(&topic, payload).await?;
        tracing::info!("Dispatched Order '{order_id}' to [{serial_number}] on '{topic}'");

        Ok(order)
    }

    /// Stores the latest VDA 5050 state snapshot, refreshes the robot's liveness ping (5s TTL),
    /// and automatically releases the previous node lock when the robot moves to a new node.
    pub async fn update_robot_state(
        &self,
        mfr: &str, // Manufacturer
        sn: &str, // Serial Number
        state: &VdaState,
    ) -> Result<(), FleetError> {
        let state_key = format!("hitkote:robot:{mfr}:{sn}:state");
        let heartbeat_key = format!("hitkote:robot:{mfr}:{sn}:heartbeat");

        // 1. Fetch previous state to detect node transitions
        if let Ok(Some(old_state)) = self.get_robot_state(mfr, sn).await {
            let old_node = &old_state.last_node_id;
            let new_node = &state.last_node_id;

            // If the robot moved to a new node, attempt to release the lock on the previous node
            if !old_node.is_empty() && old_node != new_node {
                match self.release_node_lock(old_node, sn).await {
                    Ok(true) => {
                        tracing::info!(
                            "REACTIVE RELEASE: [{sn}] moved '{old_node}' -> '{new_node}'. Released lock on '{old_node}'"
                        );
                    }
                    Ok(false) => {
                        tracing::info!(
                            "NODE TRANSITION: [{sn}] moved '{old_node}' -> '{new_node}' (No active lock held on '{old_node}')"
                        );
                    }
                    Err(err) => {
                        tracing::error!("Failed to release lock on '{old_node}' for [{sn}]: {err}");
                    }
                }
            }
        }

        let mut conn = self.redis.clone();
        let state_json = serde_json::to_string(state)?;

        // 2. Persist the full telemetry snapshot
        let _: () = conn.set(&state_key, state_json).await?;

        // 3. Touch the heartbeat key with a 5-second TTL lease
        let _: () = conn.set_ex(&heartbeat_key, "ONLINE", 5).await?;

        Ok(())
    }

    /// Fetches the latest telemetry snapshot for a specific robot.
    pub async fn get_robot_state(
        &self,
        mfr: &str,
        sn: &str,
    ) -> Result<Option<VdaState>, FleetError> {
        let state_key = format!("hitkote:robot:{mfr}:{sn}:state");
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
        let lock_key = format!("hitkote:traffic:node:{node_id}");
        let mut conn = self.redis.clone();

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
        let lock_key = format!("hitkote:traffic:node:{node_id}");
        let mut conn = self.redis.clone();

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

    /// Atomically reserves an entire path (list of nodes) for a robot using a Redis Lua script.
    /// If ANY node is already held by another robot, the entire operation aborts (returns false).
    pub async fn try_reserve_path(
        &self,
        nodes: &[&str],
        robot_id: &str,
        ttl_secs: u64,
    ) -> Result<bool, FleetError> {
        let lua_script = redis::Script::new(
            r#"
            local robot_id = ARGV[1]
            local ttl = tonumber(ARGV[2])

            -- Phase 1: Check if any node is occupied by another robot
            for i, key in ipairs(KEYS) do
                local owner = redis.call("get", key)
                if owner and owner ~= robot_id then
                    return 0
                end
            end

            -- Phase 2: Acquire locks on all nodes
            for i, key in ipairs(KEYS) do
                redis.call("set", key, robot_id, "EX", ttl)
            end

            return 1
            "#,
        );

        let keys: Vec<String> = nodes
            .iter()
            .map(|node_id| format!("hitkote:traffic:node:{node_id}"))
            .collect();

        let mut conn = self.redis.clone();
        let result: i32 = lua_script
            .key(&keys)
            .arg(robot_id)
            .arg(ttl_secs)
            .invoke_async(&mut conn)
            .await?;

        Ok(result == 1)
    }

    /// Atomically releases all nodes in a path owned by the specified robot.
    pub async fn release_path(
        &self,
        nodes: &[&str],
        robot_id: &str,
    ) -> Result<u32, FleetError> {
        let lua_script = redis::Script::new(
            r#"
            local robot_id = ARGV[1]
            local released_count = 0

            for i, key in ipairs(KEYS) do
                local owner = redis.call("get", key)
                if owner == robot_id then
                    redis.call("del", key)
                    released_count = released_count + 1
                end
            end

            return released_count
            "#,
        );

        let keys: Vec<String> = nodes
            .iter()
            .map(|node_id| format!("hitkote:traffic:node:{node_id}"))
            .collect();

        let mut conn = self.redis.clone();
        let released_count: u32 = lua_script
            .key(&keys)
            .arg(robot_id)
            .invoke_async(&mut conn)
            .await?;

        Ok(released_count)
    }
}