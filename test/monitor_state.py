import json
import time
import zenoh

def listener(sample):
    print(f"\n[Key Expression: {sample.key_expr}]")
    try:
        # Convert Zenoh payload bytes to UTF-8 JSON
        raw_data = sample.payload.to_bytes().decode('utf-8')
        json_data = json.loads(raw_data)
        print(json.dumps(json_data, indent=2))
    except Exception as e:
        print(f"Raw Payload: {sample.payload.to_bytes()}")

if __name__ == "__main__":
    conf = zenoh.Config()
    # Connect to your Zenoh router/endpoint
    conf.insert_json5("connect/endpoints", '["tcp/127.0.0.1:7447"]')
    
    print("Opening Zenoh session...")
    session = zenoh.open(conf)
    
    # Subscribe to all key expressions published by amr_01
    print("Subscribing to '**'...")
    sub = session.declare_subscriber("**", listener)
    
    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        print("\nClosing session.")
        session.close()