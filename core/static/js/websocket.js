export function connectWebSocket({ onOpen, onClose, onMessage }) {
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const wsUrl = `${protocol}//${window.location.host}/api/v1/ws`;

    const ws = new WebSocket(wsUrl);

    ws.onopen = () => {
        console.log("⚡ Connected to HITKOTE WebSocket stream");
        if (onOpen) onOpen();
    };

    ws.onmessage = (event) => {
        if (onMessage) onMessage(event.data);
    };

    ws.onclose = () => {
        console.warn("WebSocket closed. Reconnecting in 2s...");
        if (onClose) onClose();
        setTimeout(() => connectWebSocket({ onOpen, onClose, onMessage }), 2000);
    };

    ws.onerror = (err) => {
        console.error("WebSocket error:", err);
        ws.close();
    };

    return ws;
}