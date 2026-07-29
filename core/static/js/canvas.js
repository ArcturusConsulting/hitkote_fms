export class MapRenderer {
    constructor(canvasId) {
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
                // Set initial canvas drawing buffer size to match image dimensions
                this.canvas.width = this.canvas.parentElement.clientWidth || 800;
                this.canvas.height = this.canvas.parentElement.clientHeight || 600;

                // Automatically fit map into the container view on first load
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
        this.scale = Math.min(hRatio, vRatio) * 0.9; // 90% fit view
        
        // Center the map
        this.offsetX = (this.canvas.width - this.mapImage.width * this.scale) / 2;
        this.offsetY = (this.canvas.height - this.mapImage.height * this.scale) / 2;
    }

    initInteractionListeners() {
        // 1. Mouse Down (Start Drag / Pan)
        this.canvas.addEventListener('mousedown', (e) => {
            this.isDragging = true;
            this.startX = e.clientX - this.offsetX;
            this.startY = e.clientY - this.offsetY;
        });

        // 2. Mouse Move (Pan Canvas)
        window.addEventListener('mousemove', (e) => {
            if (!this.isDragging) return;
            this.offsetX = e.clientX - this.startX;
            this.offsetY = e.clientY - this.startY;
        });

        // 3. Mouse Up / Leave (Stop Drag)
        window.addEventListener('mouseup', () => { this.isDragging = false; });
        this.canvas.addEventListener('mouseleave', () => { this.isDragging = false; });

        // 4. Mouse Wheel (Zoom In / Out centered on mouse pointer)
        this.canvas.addEventListener('wheel', (e) => {
            e.preventDefault();
            if (!this.mapLoaded) return;

            const zoomFactor = 1.15;
            const rect = this.canvas.getBoundingClientRect();
            const mouseX = e.clientX - rect.left;
            const mouseY = e.clientY - rect.top;

            // Determine zoom direction
            const oldScale = this.scale;
            if (e.deltaY < 0) {
                this.scale *= zoomFactor; // Zoom in
            } else {
                this.scale /= zoomFactor; // Zoom out
            }

            // Clamp zoom bounds (min fit view, max 10x zoom)
            this.scale = Math.max(0.05, Math.min(this.scale, 10.0));

            // Adjust offset so zoom focuses on mouse cursor position
            this.offsetX = mouseX - (mouseX - this.offsetX) * (this.scale / oldScale);
            this.offsetY = mouseY - (mouseY - this.offsetY) * (this.scale / oldScale);
        }, { passive: false });
    }

    worldToCanvas(x, y) {
        if (!this.mapLoaded) return { cx: 0, cy: 0 };
        
        // 1. Convert ROS meters to raw map image pixels
        const imgPx = (x - this.originX) / this.resolution;
        const imgPy = this.mapImage.height - (y - this.originY) / this.resolution;
        
        // 2. Apply viewport zoom scale and pan offset
        const cx = imgPx * this.scale + this.offsetX;
        const cy = imgPy * this.scale + this.offsetY;
        
        return { cx, cy };
    }

    draw(nodes, edges, robotState) {
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

        // Save context state for viewport transform
        ctx.save();

        // 1. Draw Map Image with current pan/zoom matrix
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

        // 4. Draw Real-Time AGV Robot
        if (robotState) {
            let rx, ry, rtheta = 0;

            if (typeof robotState === 'object' && robotState.x !== undefined && robotState.y !== undefined) {
                rx = robotState.x;
                ry = robotState.y;
                rtheta = robotState.theta || 0;
            } else if (nodes[robotState]) {
                rx = nodes[robotState].x;
                ry = nodes[robotState].y;
            }

            if (rx !== undefined && ry !== undefined) {
                const { cx, cy } = this.worldToCanvas(rx, ry);
                const markerRadius = Math.max(8, 12 * this.scale);

                // Pulse Glow
                ctx.fillStyle = "rgba(158, 206, 106, 0.3)";
                ctx.beginPath();
                ctx.arc(cx, cy, markerRadius * 1.6, 0, Math.PI * 2);
                ctx.fill();

                // Robot Marker Body
                ctx.fillStyle = "#9ece6a";
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

                ctx.fillStyle = "#1a1b26";
                ctx.font = "bold 8px sans-serif";
                ctx.textAlign = "center";
                ctx.textBaseline = "middle";
                ctx.fillText("AGV", cx, cy);
            }
        }

        ctx.restore();
    }
}