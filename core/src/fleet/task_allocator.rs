use crate::fleet::fleet_manager::FleetManager;
use crate::router::TopologicalRouter;
use crate::vda5050::State as VdaState;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct TransportTaskRequest {
    pub task_id: Option<String>,
    pub from_node: String,
    pub to_node: String,
    pub priority: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct AllocationResult {
    pub task_id: String,
    pub assigned_robot: String,
    pub manufacturer: String,
    pub order_id: String,
}

pub struct TaskAllocator;

impl TaskAllocator {
    pub async fn allocate_and_dispatch(
        redis: &mut redis::aio::ConnectionManager,
        fleet_manager: &FleetManager,
        zenoh: &zenoh::Session,
        router: &TopologicalRouter,
        request: TransportTaskRequest,
    ) -> Result<AllocationResult, String> {
        let task_id = request
            .task_id
            .unwrap_or_else(|| format!("TASK-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]));

        // 1. Fetch active state keys (targeting telemetry keys specifically)
        let keys: Vec<String> = redis
            .keys("issem:robot:*:state") // Adjust to "issem:robot:*" if you don't use the :state suffix
            .await
            .map_err(|e| format!("Redis scan error: {e}"))?;

        if keys.is_empty() {
            return Err("No active robots found in fleet registry.".into());
        }

        let mut best_candidate: Option<(String, String, String, f64)> = None; // (mfg, serial, current_node, distance_m)

        // 2. Evaluate candidates
        for key in keys {
            let raw_json: Option<String> = redis
                .get(&key)
                .await
                .map_err(|e| format!("Failed to read robot data for {key}: {e}"))?;

            let json_str = match raw_json {
                Some(j) => j,
                None => continue,
            };

            let state: VdaState = match serde_json::from_str(&json_str) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Failed to parse VdaState for key {key}: {e}");
                    continue;
                }
            };

            // Battery check
            let battery = state.battery_state.battery_charge as f32;
            if battery < 20.0 {
                continue;
            }

            // Availability check (must not be driving or executing an active order)
            if state.driving || !state.order_id.is_empty() {
                continue;
            }

            // Position check
            let current_node = &state.last_node_id;
            if current_node.is_empty() {
                continue;
            }

            let mfg = state.header.manufacturer.clone();
            let serial = state.header.serial_number.clone();

            // 3. Evaluate shortest path from candidate to task starting node
            if let Some(route_plan) = router.find_path(current_node, &request.from_node) {
                let dist = route_plan.total_distance_m;

                match &best_candidate {
                    Some((_, _, _, min_dist)) if dist < *min_dist => {
                        best_candidate = Some((mfg, serial, current_node.clone(), dist));
                    }
                    None => {
                        best_candidate = Some((mfg, serial, current_node.clone(), dist));
                    }
                    _ => {}
                }
            }
        }

        // 4. Dispatch task to closest available candidate
        if let Some((mfg, serial, start_node, _)) = best_candidate {
            let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
            let short_hash = &uuid::Uuid::new_v4().simple().to_string()[..4];
            let order_id = format!("ORD-{timestamp}-{serial}-{short_hash}");

            // Dispatch order via Zenoh (robot updates its own state in Redis upon receipt)
            fleet_manager
                .dispatch_order(
                    zenoh,
                    router,
                    &mfg,
                    &serial,
                    &start_node,
                    &request.to_node,
                    &order_id,
                    60,
                )
                .await
                .map_err(|e| format!("Dispatch error: {e:?}"))?;

            Ok(AllocationResult {
                task_id,
                assigned_robot: serial,
                manufacturer: mfg,
                order_id,
            })
        } else {
            Err("No available IDLE robot could be allocated for this task.".into())
        }
    }
}