import { MapRenderer } from './canvas.js';
import { connectWebSocket } from './websocket.js';

// Map Topology Definition (Matches main.rs)
const NODES = {
    "node_A1": { x: 0.0,  y: 0.0 },
    "node_A2": { x: 5.0,  y: 0.0 },
    "node_A3": { x: 10.0, y: 0.0 }
};

const EDGES = [
    ["node_A1", "node_A2"],
    ["node_A2", "node_A3"]
];

let activeRobotNode = "node_A1";

// DOM Elements
const indicator = document.getElementById("status-indicator");
const statusText = document.getElementById("status-text");
const telemetryLog = document.getElementById("telemetry-log");
const nodeDisplay = document.getElementById("robot-node");
const seqDisplay = document.getElementById("robot-seq");

// Renderer Initialization
const renderer = new MapRenderer("mapCanvas");
renderer.draw(NODES, EDGES, activeRobotNode);

// Connect WebSocket & Register Event Handlers
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
            
            // Format raw JSON display
            telemetryLog.innerText = JSON.stringify(data, null, 2);

            // Update Active Robot Node
            if (data.lastNodeId) {
                activeRobotNode = data.lastNodeId;
                nodeDisplay.innerText = data.lastNodeId;
                
                // Re-render map canvas with updated position
                renderer.draw(NODES, EDGES, activeRobotNode);
            }

            if (data.headerId !== undefined) {
                seqDisplay.innerText = data.headerId;
            }
        } catch (e) {
            telemetryLog.innerText = rawMessage;
        }
    }
});