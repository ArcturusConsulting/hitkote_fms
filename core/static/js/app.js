import { MapRenderer } from './canvas.js';
import { connectWebSocket } from './websocket.js';

// Dynamic Topology Containers & Fleet State Map
let NODES = {};
let EDGES = [];
let fleetState = {}; // Keyed by robot serial/id: { "agv_1": { x, y, theta, lastNodeId, rawData } }
let selectedRobotId = null;

// DOM Elements
const indicator = document.getElementById("status-indicator");
const statusText = document.getElementById("status-text");
const telemetryLog = document.getElementById("telemetry-log");
const nodeDisplay = document.getElementById("robot-node");
const seqDisplay = document.getElementById("robot-seq");
const activeTitle = document.getElementById("active-robot-title");

// Initialize Canvas Renderer with Selection Handler
const renderer = new MapRenderer("mapCanvas", (clickedId) => {
    selectedRobotId = clickedId;
    updateSidebarUI();
});

// Fetch Dynamic Topology Graph
async function loadGraphTopology() {
    try {
        const response = await fetch('/assets/graph.json');
        if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
        const graphData = await response.json();
        
        NODES = graphData.nodes || {};
        EDGES = (graphData.edges || []).map(edge => [edge[0], edge[1]]);
        console.log("Dynamic topology loaded:", NODES, EDGES);
    } catch (err) {
        console.warn("No graph.json found or failed to parse. Rendering map without topology overlays.", err);
    }
}

// Boot Graph Loading
loadGraphTopology();

// Main Render Loop (10 Hz)
setInterval(() => {
    renderer.draw(NODES, EDGES, fleetState, selectedRobotId);
}, 100);

// Helper function to update sidebar for selected robot
function updateSidebarUI() {
    if (!selectedRobotId || !fleetState[selectedRobotId]) {
        if (activeTitle) activeTitle.innerText = "Active Robot State (None)";
        nodeDisplay.innerText = "--";
        seqDisplay.innerText = "--";
        telemetryLog.innerText = "Click on a robot on the map to inspect telemetry.";
        return;
    }

    if (activeTitle) activeTitle.innerText = `Active Robot State [ ${selectedRobotId} ]`;
    
    const currentRobot = fleetState[selectedRobotId];
    telemetryLog.innerText = JSON.stringify(currentRobot.rawData, null, 2);

    if (currentRobot.x !== undefined && currentRobot.y !== undefined) {
        nodeDisplay.innerText = `(${currentRobot.x.toFixed(2)}, ${currentRobot.y.toFixed(2)})`;
    } else if (currentRobot.lastNodeId) {
        nodeDisplay.innerText = currentRobot.lastNodeId;
    }

    seqDisplay.innerText = currentRobot.headerId !== undefined ? currentRobot.headerId : "--";
}

// Connect WebSocket & Process Multi-Robot Telemetry
connectWebSocket({
    onOpen: () => {
        indicator.classList.add("online");
        statusText.innerText = "Connected";
    },
    onClose: () => {
        indicator.classList.remove("online");
        statusText.innerText = "Disconnected (Retrying...)";
    },
    onMessage: (rawMessage) => {
        try {
            const data = JSON.parse(rawMessage);

            // 1. Extract AGV Identifier (handles nested header, top-level, or fallback)
            const robotId = 
                data.header?.serialNumber || 
                data.serialNumber || 
                data.robot_id || 
                data.agvId || 
                "AGV";

            // Automatically select the first reporting robot
            if (!selectedRobotId || selectedRobotId === "AGV") {
                selectedRobotId = robotId;
            }

            // 2. Extract Position Info (handles agvPosition, position, x/y, or node fallback)
            let posX, posY, posTheta;
            const pos = data.agvPosition || data.position || data.agv_position;

            if (pos && pos.x !== undefined && pos.y !== undefined) {
                posX = pos.x;
                posY = pos.y;
                posTheta = pos.theta || 0;
            } else if (data.x !== undefined && data.y !== undefined) {
                posX = data.x;
                posY = data.y;
                posTheta = data.theta || 0;
            } else if (data.lastNodeId && NODES[data.lastNodeId]) {
                // Fall back to graph node position if continuous x/y is missing
                posX = NODES[data.lastNodeId].x;
                posY = NODES[data.lastNodeId].y;
                posTheta = 0;
            }

            // 3. Store state under correct robot key (e.g., fleetState["amr_01"])
            fleetState[robotId] = {
                x: posX,
                y: posY,
                theta: posTheta,
                lastNodeId: data.lastNodeId || (data.nodes && data.nodes[0]?.nodeId),
                headerId: data.header?.headerId || data.headerId,
                rawData: data
            };

            // 4. Refresh UI sidebar if active robot changed state
            if (robotId === selectedRobotId) {
                updateSidebarUI();
            }

        } catch (e) {
            console.error("Failed to parse WebSocket message:", e);
            if (telemetryLog) telemetryLog.innerText = rawMessage;
        }
    }
});