import { MapRenderer } from './canvas.js';
import { connectWebSocket } from './websocket.js';

// Dynamic Topology Containers (Populated from graph.json)
let NODES = {};
let EDGES = [];
let robotState = { x: 0.0, y: 0.0, theta: 0.0 };

// DOM Elements
const indicator = document.getElementById("status-indicator");
const statusText = document.getElementById("status-text");
const telemetryLog = document.getElementById("telemetry-log");
const nodeDisplay = document.getElementById("robot-node");
const seqDisplay = document.getElementById("robot-seq");

// Initialize Canvas Renderer
const renderer = new MapRenderer("mapCanvas");

// Fetch Dynamic Topology Graph
async function loadGraphTopology() {
    try {
        const response = await fetch('/assets/graph.json');
        if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
        const graphData = await response.json();
        
        NODES = graphData.nodes || {};
        EDGES = (graphData.edges || []).map(edge => [edge[0], edge[1]]);
        console.log("✅ Dynamic topology loaded:", NODES, EDGES);
    } catch (err) {
        console.warn("⚠️ No graph.json found or failed to parse. Rendering map without topology overlays.", err);
    }
}

// Boot Graph Loading
loadGraphTopology();

// Main Render Loop (10 Hz)
setInterval(() => {
    renderer.draw(NODES, EDGES, robotState);
}, 100);

// Connect WebSocket & Process Telemetry
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
            telemetryLog.innerText = JSON.stringify(data, null, 2);

            // 1. Process VDA 5050 Continuous Pose (x, y, theta)
            if (data.agvPosition && data.agvPosition.x !== undefined) {
                robotState = { 
                    x: data.agvPosition.x, 
                    y: data.agvPosition.y, 
                    theta: data.agvPosition.theta || 0 
                };
                nodeDisplay.innerText = `(${data.agvPosition.x.toFixed(2)}, ${data.agvPosition.y.toFixed(2)})`;
            } else if (data.x !== undefined && data.y !== undefined) {
                robotState = { x: data.x, y: data.y, theta: data.theta || 0 };
                nodeDisplay.innerText = `(${data.x.toFixed(2)}, ${data.y.toFixed(2)})`;
            } 
            // 2. Fallback to Discrete Node ID
            else if (data.lastNodeId) {
                robotState = data.lastNodeId;
                nodeDisplay.innerText = data.lastNodeId;
            }

            if (data.headerId !== undefined) {
                seqDisplay.innerText = data.headerId;
            }
        } catch (e) {
            telemetryLog.innerText = rawMessage;
        }
    }
});