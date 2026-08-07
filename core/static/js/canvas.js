export class MapRenderer {
    constructor(canvasId, onRobotSelect = null) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext("2d");
        
        // Map metadata defaults
        this.resolution = 0.05;
        this.originX = 0.0;
        this.originY = 0.0;

        this.mapImage = new Image();
        this.mapLoaded = false;

        // Pan & Zoom State Variables
        this.scale = 1.0;         // Zoom multiplier
        this.offsetX = 0.0;       // Pan offset X in pixels
        this.offsetY = 0.0;       // Pan offset Y in pixels
        this.isDragging = false;
        this.startX = 0;
        this.startY = 0;
        this.hasDragged = false; // Distinguish clicks from dragging

        // Callback when a robot is clicked on the map
        this.onRobotSelect = onRobotSelect;

        // Internal reference to current fleet for hit testing
        this.currentFleet = {};
        this.currentNodes = {};

        this.initMap();
        this.initInteractionListeners();
    }

    async initMap() {
        try {
            const response = await fetch('/assets/map.yaml');
            const yamlText = await response.text();

            const resMatch = yamlText.match(/resolution:\s*([0-9.]+)/);
            const originMatch = yamlText.match(/origin:\s*\[\s*([-\d.]+),\s*([-\d.]+),\s*([-\d.]+)\]/);
            const imageMatch = yamlText.match(/image:\s*(.+)/);

            if (resMatch) this.resolution = parseFloat(resMatch[1]);
            if (originMatch) {
                this.originX = parseFloat(originMatch[1]);
                this.originY = parseFloat(originMatch[2]);
            }

            let imageName = 'map.png';
            if (imageMatch) {
                const rawImage = imageMatch[1].trim().replace(/['"]+/g, '');
                imageName = rawImage.endsWith('.pgm') ? rawImage.replace('.pgm', '.png') : rawImage;
            }

            this.mapImage.src = `/assets/${imageName}`;
            this.mapImage.onload = () => {
                this.mapLoaded = true;
                this.canvas.width = this.canvas.parentElement.clientWidth || 800;
                this.canvas.height = this.canvas.parentElement.clientHeight || 600;
                this.fitMapToScreen();
            };
        } catch (err) {
            console.error("Failed to load map configuration from YAML:", err);
            this.mapImage.src = '/assets/map.png';
            this.mapImage.onload = () => {
                this.mapLoaded = true;
                this.fitMapToScreen();
            };
        }
    }

    fitMapToScreen() {
        if (!this.mapLoaded) return;
        const hRatio = this.canvas.width / this.mapImage.width;
        const vRatio = this.canvas.height / this.mapImage.height;
        this.scale = Math.min(hRatio, vRatio) * 0.9;
        
        this.offsetX = (this.canvas.width - this.mapImage.width * this.scale) / 2;
        this.offsetY = (this.canvas.height - this.mapImage.height * this.scale) / 2;
    }

    initInteractionListeners() {
        // 1. Mouse Down
        this.canvas.addEventListener('mousedown', (e) => {
            this.isDragging = true;
            this.hasDragged = false;
            this.startX = e.clientX - this.offsetX;
            this.startY = e.clientY - this.offsetY;
        });

        // 2. Mouse Move
        window.addEventListener('mousemove', (e) => {
            if (!this.isDragging) return;
            this.hasDragged = true; // User is panning, so do not trigger click/selection
            this.offsetX = e.clientX - this.startX;
            this.offsetY = e.clientY - this.startY;
        });

        // 3. Mouse Up / Click Selection
        window.addEventListener('mouseup', (e) => { 
            if (this.isDragging && !this.hasDragged && e.target === this.canvas) {
                this.handleMapClick(e);
            }
            this.isDragging = false; 
        });

        this.canvas.addEventListener('mouseleave', () => { this.isDragging = false; });

        // 4. Mouse Wheel (Zoom)
        this.canvas.addEventListener('wheel', (e) => {
            e.preventDefault();
            if (!this.mapLoaded) return;

            const zoomFactor = 1.15;
            const rect = this.canvas.getBoundingClientRect();
            const mouseX = e.clientX - rect.left;
            const mouseY = e.clientY - rect.top;

            const oldScale = this.scale;
            if (e.deltaY < 0) {
                this.scale *= zoomFactor;
            } else {
                this.scale /= zoomFactor;
            }

            this.scale = Math.max(0.05, Math.min(this.scale, 10.0));

            this.offsetX = mouseX - (mouseX - this.offsetX) * (this.scale / oldScale);
            this.offsetY = mouseY - (mouseY - this.offsetY) * (this.scale / oldScale);
        }, { passive: false });
    }

    handleMapClick(e) {
        const rect = this.canvas.getBoundingClientRect();
        const clickX = e.clientX - rect.left;
        const clickY = e.clientY - rect.top;

        const clickedRobotId = this.getRobotAtCanvasCoords(clickX, clickY);
        if (this.onRobotSelect) {
            this.onRobotSelect(clickedRobotId); // Pass selected robot ID or null
        }
    }

    getRobotAtCanvasCoords(clickX, clickY) {
        for (const [robotId, state] of Object.entries(this.currentFleet)) {
            let rx, ry;
            if (state.x !== undefined && state.y !== undefined) {
                rx = state.x;
                ry = state.y;
            } else if (typeof state === 'string' && this.currentNodes[state]) {
                rx = this.currentNodes[state].x;
                ry = this.currentNodes[state].y;
            }

            if (rx !== undefined && ry !== undefined) {
                const { cx, cy } = this.worldToCanvas(rx, ry);
                const markerRadius = Math.max(8, 12 * this.scale);
                
                // Hit threshold radius (clickable area)
                const distance = Math.hypot(clickX - cx, clickY - cy);
                if (distance <= markerRadius * 1.8) {
                    return robotId;
                }
            }
        }
        return null;
    }

    worldToCanvas(x, y) {
        if (!this.mapLoaded) return { cx: 0, cy: 0 };
        
        const imgPx = (x - this.originX) / this.resolution;
        const imgPy = this.mapImage.height - (y - this.originY) / this.resolution;
        
        const cx = imgPx * this.scale + this.offsetX;
        const cy = imgPy * this.scale + this.offsetY;
        
        return { cx, cy };
    }

    draw(nodes, edges, fleetState, selectedRobotId = null) {
        this.currentNodes = nodes;
        this.currentFleet = fleetState || {};

        const { ctx, canvas } = this;
        ctx.clearRect(0, 0, canvas.width, canvas.height);

        if (!this.mapLoaded) {
            ctx.fillStyle = "#1a1b26";
            ctx.fillRect(0, 0, canvas.width, canvas.height);
            ctx.fillStyle = "#a9b1d6";
            ctx.font = "16px monospace";
            ctx.textAlign = "center";
            ctx.fillText("Loading map & YAML config...", canvas.width / 2, canvas.height / 2);
            return;
        }

        ctx.save();

        // 1. Draw Map Image
        ctx.drawImage(
            this.mapImage, 
            this.offsetX, 
            this.offsetY, 
            this.mapImage.width * this.scale, 
            this.mapImage.height * this.scale
        );

        // 2. Draw Topology Edges
        ctx.strokeStyle = "#414868";
        ctx.lineWidth = Math.max(2, 3 * this.scale);
        edges.forEach(([from, to]) => {
            if (nodes[from] && nodes[to]) {
                const p1 = this.worldToCanvas(nodes[from].x, nodes[from].y);
                const p2 = this.worldToCanvas(nodes[to].x, nodes[to].y);
                ctx.beginPath();
                ctx.moveTo(p1.cx, p1.cy);
                ctx.lineTo(p2.cx, p2.cy);
                ctx.stroke();
            }
        });

        // 3. Draw Topology Nodes
        Object.entries(nodes).forEach(([nodeId, pos]) => {
            const { cx, cy } = this.worldToCanvas(pos.x, pos.y);

            ctx.fillStyle = "#2ac3de";
            ctx.beginPath();
            ctx.arc(cx, cy, Math.max(4, 6 * this.scale), 0, Math.PI * 2);
            ctx.fill();

            ctx.fillStyle = "#c0caf5";
            ctx.font = "10px monospace";
            ctx.textAlign = "center";
            ctx.fillText(nodeId, cx, cy - (12 * this.scale));
        });

        // 4. Draw Fleet Robots
        Object.entries(this.currentFleet).forEach(([robotId, state]) => {
            let rx, ry, rtheta = 0;

            if (state && state.x !== undefined && state.y !== undefined) {
                rx = state.x;
                ry = state.y;
                rtheta = state.theta || 0;
            } else if (typeof state === 'string' && nodes[state]) {
                rx = nodes[state].x;
                ry = nodes[state].y;
            }

            if (rx !== undefined && ry !== undefined) {
                const { cx, cy } = this.worldToCanvas(rx, ry);
                const markerRadius = Math.max(8, 12 * this.scale);
                const isSelected = robotId === selectedRobotId;

                // Active Selection Highlight Ring
                if (isSelected) {
                    ctx.strokeStyle = "#7aa2f7";
                    ctx.lineWidth = 3;
                    ctx.beginPath();
                    ctx.arc(cx, cy, markerRadius * 2.2, 0, Math.PI * 2);
                    ctx.stroke();
                }

                // Pulse Glow
                ctx.fillStyle = isSelected ? "rgba(122, 162, 247, 0.4)" : "rgba(158, 206, 106, 0.3)";
                ctx.beginPath();
                ctx.arc(cx, cy, markerRadius * 1.6, 0, Math.PI * 2);
                ctx.fill();

                // Robot Marker Body
                ctx.fillStyle = isSelected ? "#7aa2f7" : "#9ece6a";
                ctx.beginPath();
                ctx.arc(cx, cy, markerRadius, 0, Math.PI * 2);
                ctx.fill();

                // Heading Arrow Direction
                ctx.strokeStyle = "#1a1b26";
                ctx.lineWidth = Math.max(1.5, 3 * this.scale);
                ctx.beginPath();
                ctx.moveTo(cx, cy);
                const headX = cx + (markerRadius * 1.5) * Math.cos(-rtheta);
                const headY = cy + (markerRadius * 1.5) * Math.sin(-rtheta);
                ctx.lineTo(headX, headY);
                ctx.stroke();

                // Label (Robot ID / AGV)
                ctx.fillStyle = "#1a1b26";
                ctx.font = "bold 8px sans-serif";
                ctx.textAlign = "center";
                ctx.textBaseline = "middle";
                ctx.fillText(robotId.substring(0, 5), cx, cy);
            }
        });

        ctx.restore();
    }
}