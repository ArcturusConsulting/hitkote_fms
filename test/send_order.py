import argparse
from datetime import datetime, timezone
import json
import time
import zenoh

def build_vda5050_order(agv_id: str, order_id: str) -> dict:
    """Build a minimal valid VDA 5050 v3 Order payload."""
    timestamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    
    return {
        "headerId": 1,
        "timestamp": timestamp,
        "version": "3.0.0",
        "manufacturer": "ISSEM",
        "serialNumber": agv_id,
        "orderId": order_id,
        "orderUpdateId": 0,
        "nodes": [
            {
                "nodeId": "start_node",
                "sequenceId": 0,
                "released": True,
                "nodePosition": {
                    "x": 0.0,
                    "y": 0.0,
                    "theta": 0.0,
                    "mapId": "map",
                    "positionInitialized": True
                },
                "actions": []
            },
            {
                "nodeId": "target_node",
                "sequenceId": 2,
                "released": True,
                "nodePosition": {
                    "x": 2.5,
                    "y": 1.0,
                    "theta": 1.57,
                    "mapId": "map",
                    "positionInitialized": True
                },
                "actions": []
            }
        ],
        "edges": [
            {
                "edgeId": "edge_start_to_target",
                "sequenceId": 1,
                "released": True,
                "startNodeId": "start_node",
                "endNodeId": "target_node",
                "actions": []
            }
        ]
    }

def main():
    parser = argparse.ArgumentParser(description="Publish a VDA 5050 Order over Zenoh.")
    parser.add_argument("--agv-id", default="amr_01", help="Target AGV serial number")
    parser.add_argument("--order-id", default="order_001", help="Unique order identifier")
    args = parser.parse_args()

    key_expr = f"vda5050/v3/issem/{args.agv_id}/order"
    order_payload = build_vda5050_order(args.agv_id, args.order_id)

    print(f"Opening Zenoh session...", flush=True)
    conf = zenoh.Config()
    session = zenoh.open(conf)

    print(f"Publishing VDA 5050 Order to key expression: {key_expr}\n", flush=True)
    session.put(key_expr, json.dumps(order_payload))

    print(json.dumps(order_payload, indent=2), flush=True)

    # Allow buffer to flush to network before closing
    time.sleep(1)
    session.close()
    print("\nDone.", flush=True)

if __name__ == "__main__":
    main()