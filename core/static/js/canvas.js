export class MapRenderer {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext("2d");
        this.scale = 50; // 50px per meter
        this.offsetX = 150;
        this.offsetY = 250;
    }

    worldToCanvas(x, y) {
        return {
            cx: this.offsetX + x * this.scale,
            cy: this.offsetY - y * this.scale // Invert Y axis
        };
    }

    draw(nodes, edges, activeRobotNode) {
        const { ctx, canvas } = this;
        ctx.clearRect(0, 0, canvas.width, canvas.height);

        // 1. Draw Edges
        ctx.strokeStyle = "#414868";
        ctx.lineWidth = 4;
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

        // 2. Draw Nodes
        Object.entries(nodes).forEach(([nodeId, pos]) => {
            const { cx, cy } = this.worldToCanvas(pos.x, pos.y);

            ctx.fillStyle = "#2ac3de";
            ctx.beginPath();
            ctx.arc(cx, cy, 10, 0, Math.PI * 2);
            ctx.fill();

            ctx.fillStyle = "#a9b1d6";
            ctx.font = "12px monospace";
            ctx.textAlign = "center";
            ctx.fillText(nodeId, cx, cy + 25);
        });

        // 3. Draw Active AGV Robot
        if (nodes[activeRobotNode]) {
            const { cx, cy } = this.worldToCanvas(nodes[activeRobotNode].x, nodes[activeRobotNode].y);

            // Outer Pulse
            ctx.fillStyle = "rgba(158, 206, 106, 0.25)";
            ctx.beginPath();
            ctx.arc(cx, cy, 22, 0, Math.PI * 2);
            ctx.fill();

            // AGV Marker
            ctx.fillStyle = "#9ece6a";
            ctx.beginPath();
            ctx.arc(cx, cy, 14, 0, Math.PI * 2);
            ctx.fill();

            ctx.fillStyle = "#1a1b26";
            ctx.font = "bold 10px sans-serif";
            ctx.textAlign = "center";
            ctx.textBaseline = "middle";
            ctx.fillText("AGV", cx, cy);
        }
    }
}