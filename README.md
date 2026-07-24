![ISSEM Logo](ISSEM.png)

# ISSEM FMS (Intralogistics System Smart Edge Manager)

An open-source, vendor-agnostic Fleet Management System (FMS) built on **Eclipse Zenoh** and **VDA 5050 v3.0.0**. Designed to orchestrate heterogeneous fleets of Autonomous Mobile Robots (AMRs) and Automated Guided Vehicles (AGVs) in high-density warehouse and manufacturing environments.

---

## 🛑 The Problem: Vendor Lock-In & Unreliable Wireless

Traditional intralogistics relies heavily on proprietary Systems Integrator (SI) platforms and single-vendor hardware ecosystems:
* **Vendor Lock-In:** Integrating AMRs from Manufacturer A with AGVs from Manufacturer B requires costly custom middleware or entirely separate software silos.
* **Wi-Fi TCP Bottlenecks:** Standard MQTT/TCP protocols suffer from Head-of-Line blocking and packet storms when robots roam behind steel pillars or switch Wi-Fi Access Points.
* **Heavy Edge Overhead:** Attempting to control fleet movement using raw, high-frequency internal ROS 2 topics (`/cmd_vel`, `/tf`, `/scan`) over wireless networks wastes bandwidth and computational resources.

---

## 🚀 The ISSEM Solution

ISSEM FMS provides a **local-first, open-source orchestration core** that bridges high-level enterprise logistics with low-level robot execution:

1. **VDA 5050 v3.0.0 Native:** Adopts the latest international standard interface for AMRs/AGVs, supporting free navigation, zone-based traffic rules, retriable actions, and path sharing.
2. **Zenoh Transport Layer:** Replaces standard TCP/MQTT with Zenoh (UDP/multicast). Eliminates Wi-Fi drop-out stalls, operates with micro-byte headers, and scales peer-to-peer across warehouse subnets.
3. **Decoupled Architecture:** High-level topological dispatching runs in a high-performance **Rust** core, while a lightweight **C++ ROS 2 Bridge** handles edge translation on the robot.
4. **Fail-Safe Durable State:** Utilizes **Redis** with AOF persistence for active telemetry snapshots, order recovery, and leased TTL spatial traffic locks (3,000 ms) to prevent warehouse deadlocks upon system crashes.
5. **Air-Gapped & Local-First:** Runs 100% locally within warehouse infrastructure via Docker Compose—ideal for strict enterprise security compliance.

---

## 📐 System Architecture

```text
+-----------------------------------------------------------------------+
|                       WMS / ERP / Enterprise Systems                  |
+-----------------------------------------------------------------------+
                                   |  HTTP POST /api/v1/orders
                                   v
+-----------------------------------------------------------------------+
|                         ISSEM FMS CORE (Rust)                         |
|  - Axum Web Server (REST API & WebSockets Dashboard on Port 8080)     |
|  - Petgraph Topological Router (A* / Dijkstra Pathfinding)            |
|  - Dynamic Node & Zone Traffic Reservation Manager                    |
|  - Zenoh Router (`zenoh-rs`)                                          |
+-----------------------------------------------------------------------+
                                   |
                +------------------+------------------+
                |                                     |
                v                                     v
+----------------------------------+ +----------------------------------+
|  REDIS 7 STATE STORE & LOCKS     | |  ZENOH BUS (UDP / Multicast)    |
|  - `issem:robot:*` Telemetry     | |  Pattern: `issem/v3/{mfr}/{sn}/` |
|  - `issem:traffic:*` Leased Locks| |  Payload: VDA 5050 v3.0.0 JSON   |
|  - `issem:order:*` Active State  | +----------------------------------+
+----------------------------------+                  |
                                   +------------------+------------------+
                                   |                                     |
                                   v                                     v
                  +---------------------------------+   +---------------------------------+
                  |   ISSEM ROS 2 BRIDGE (C++ Node) |   |     Non-ROS 2 Legacy AGV Bridge |
                  |   - rclcpp & zenoh-cpp          |   |     - PLC / Modbus / Serial     |
                  |   - Nav2 Action Client          |   |     - VDA 5050 Translator       |
                  +---------------------------------+   +---------------------------------+
                                   |                                     |
                                   v Local ROS 2 IPC                     v Serial / Fieldbus
                  +---------------------------------+   +---------------------------------+
                  |   Modern ROS 2 AMR (Nav2 / SLAM) |   |     Legacy Industrial AGV       |
                  +---------------------------------+   +---------------------------------+
```

---

## 🛠️ Technology Stack

| Layer | Technology | Function |
|---|---|---|
| **FMS Core Engine** | **Rust** (`tokio`, `petgraph`, `serde`) | Task allocation, topological graph routing, node/zone locks |
| **Durable State Store** | **Redis 7** (AOF Enabled) | Telemetry persistence, active order state, leased TTL spatial locks |
| **API & Dashboard Server**| **Axum** (Rust) | REST API for WMS triggers + WebSockets for live 2D UI |
| **Transport Network** | **Eclipse Zenoh** (`zenoh-rs`, `zenoh-cpp`) | Low-latency, loss-resilient pub/sub & queryable transport |
| **Data Protocol** | **VDA 5050 v3.0.0** | Open JSON standard for order control, state, and visualization |
| **Edge Adapter** | **C++** (`rclcpp`, `nlohmann/json`) | Microsecond JSON parsing & ROS 2 Nav2 goal execution |
| **Robot Middleware** | **ROS 2** (Humble / Jazzy) | Onboard SLAM, local costmaps, and motor controller execution |
| **Deployment Target** | **Docker Compose** | Single-command deployment for air-gapped local warehouse servers |

---

## 📁 Repository Directory Layout

```text
issem_fms/
├── README.md                  # System overview & technical documentation
├── docker-compose.yml         # Container orchestrator (Redis 7 + ISSEM Core)
├── .gitignore                 # Root gitignore (Cargo, colcon, CMake, Docker)
│
├── core/                      # FMS SERVER ENGINE (Rust)
│   ├── Dockerfile             # Multi-stage Rust build container
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs            # Entry point & Zenoh session setup
│       ├── api/               # Axum REST endpoints & WebSocket handler
│       ├── fleet/             # Fleet manager & Redis persistence layer
│       ├── router/            # Topological graph routing & zone reservation logic
│       └── vda5050/           # Rust serde structs for VDA 5050 v3.0.0
│
├── adapter/                   # EDGE ROBOT BRIDGES
│   └── issem_ros2_bridge/     # C++ ROS 2 package
│       ├── CMakeLists.txt
│       ├── package.xml
│       └── src/
│           └── bridge_node.cpp # Zenoh subscriber <-> Nav2 Action Client bridge
│
├── common/                    # SHARED DATA SPECIFICATIONS
│   └── schemas/               # VDA 5050 v3.0.0 JSON schema validation definitions
│
└── sim/                       # SIMULATION & DEMOS
    └── launch/                # Gazebo environment & multi-robot test launches
```

---

## 🔒 Durable State & Traffic Lock Schema (Redis)

ISSEM FMS structures state in Redis using a structured key hierarchy:

| Key Pattern | Type | Purpose | Policy |
|---|---|---|---|
| `issem:robot:{mfr}:{sn}:state` | JSON | Latest VDA 5050 telemetry snapshot | Persisted (AOF) |
| `issem:robot:{mfr}:{sn}:heartbeat` | String | Liveness check | Expires in 5 seconds (`EX 5`) |
| `issem:order:{order_id}` | JSON | Active order progress & route nodes | Persisted until completion |
| `issem:traffic:node:{node_id}` | String | Exclusive topological node lease | Expires in 3,000 ms (`NX PX 3000`) |
| `issem:traffic:edge:{edge_id}` | String | Directional path segment lease | Expires in 3,000 ms (`NX PX 3000`) |

---

## 📡 Zenoh Key-Expression Scheme (VDA 5050 v3.0.0)

ISSEM FMS organizes network traffic using Zenoh's hierarchical key-expressions:

$$\text{Key Scheme: } \texttt{issem/v3/\{manufacturer\}/\{serialNumber\}/\{interface\}}}$$

* **`issem/v3/{mfr}/{sn}/order`** *(FMS $\rightarrow$ Robot)*: Assigns topological route nodes, edges, and actions.
* **`issem/v3/{mfr}/{sn}/instantActions`** *(FMS $\rightarrow$ Robot)*: Sends high-priority overrides (`pause`, `resume`, `cancelOrder`, `StartHibernation`).
* **`issem/v3/{mfr}/{sn}/state`** *(Robot $\rightarrow$ FMS)*: Periodic/event-driven state updates (last node reached, battery, errors, operating mode).
* **`issem/v3/{mfr}/{sn}/visualization`** *(Robot $\rightarrow$ FMS)*: High-frequency $(x, y, \theta)$ position streaming for 2D UI rendering.
* **`issem/v3/{mfr}/{sn}/factsheet`** *(Robot $\rightarrow$ FMS)*: Static vehicle capability metadata published upon registration.

---

## 🌐 Dual API Specification (Axum Server)

The Axum core runs both REST and WebSockets on port `8080`:

### 1. REST API (For WMS / ERP Integration)
* **`POST /api/v1/orders`** — Submit a new pickup/delivery order.
* **`GET /api/v1/orders/{id}`** — Query order execution status.
* **`GET /api/v1/fleet`** — Fetch current status of all registered AMRs/AGVs.
* **`POST /api/v1/map`** — Upload or update the warehouse topological graph.

### 2. WebSocket API (For Real-Time Dashboard)
* **`WS /ws`** — Bi-directional JSON stream. Pushes live robot positions, zone reservation states, and system alerts to the web dashboard at 1–2 Hz.

---

## ⚡ Getting Started (Quickstart)

### Prerequisites
* Rust (1.80+)
* Docker & Docker Compose
* ROS 2 (Humble or Jazzy)
* C++17 Compiler & CMake

### Running locally via Docker Compose

1. **Clone the Repository:**
   ```bash
   git clone [https://github.com/your-username/issem_fms.git](https://github.com/your-username/issem_fms.git)
   cd issem_fms
   ```

2. **Launch Redis & ISSEM Core:**
   ```bash
   docker compose up --build
   ```

3. **Verify Redis & Core Connectivity:**
   ```bash
   # Check Redis container health
   docker exec -it issem-redis redis-cli ping
   # Expected output: PONG
   ```

4. **Dispatch a Sample Order via REST:**
   ```bash
   curl -X POST http://localhost:8080/api/v1/orders \
     -H "Content-Type: application/json" \
     -d '{
       "orderId": "ORD_1001",
       "targetNode": "Station_B",
       "requiredCapability": "pallet_transport"
     }'
   ```

---

## 🗺️ Development Roadmap

- [x] Architecture design & VDA 5050 v3.0.0 schema definitions
- [x] Docker Compose environment setup (Redis 7 + Core Container)
- [x] Redis Durable State & Leased Spatial Lock Schema design
- [ ] **Phase 1:** Core Zenoh router setup & C++ ROS 2 Nav2 bridge implementation
- [x] **Phase 2:** Redis integration in `fleet.rs` (`redis-rs` async client)
- [ ] **Phase 3:** Topological graph routing engine (`petgraph` + A* search)
- [x] **Phase 4:** Node & Zone reservation manager with atomic Lua scripts
- [ ] **Phase 5:** Axum REST & WebSocket API layer with 2D Canvas web dashboard
- [ ] **Phase 6:** Multi-robot Gazebo simulation integration test suite

---

## 📄 License

Distributed under the Apache 2.0 License. See `LICENSE` for details.