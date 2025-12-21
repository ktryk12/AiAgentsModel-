import sys
import time
import json
import argparse

def emit(event_type, **kwargs):
    msg = {"type": event_type}
    msg.update(kwargs)
    print(json.dumps(msg), flush=True)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--job-id", required=True)
    parser.add_argument("--out-dir", required=True)
    # ignore other args
    args, _ = parser.parse_known_args()

    # Simulate lifecycle
    time.sleep(1)
    emit("start", ts=int(time.time()))
    
    time.sleep(1)
    emit("loading_base")
    
    time.sleep(1)
    emit("loading_dataset")
    
    for i in range(3):
        time.sleep(1)
        # emit progress
        emit("progress", epoch=1, step=i*10, loss=2.5 - i*0.5)

    time.sleep(1)
    emit("saving")
    
    time.sleep(1)
    # Create fake adapter
    import os
    os.makedirs(args.out_dir, exist_ok=True)
    with open(os.path.join(args.out_dir, "adapter_config.json"), "w") as f:
        f.write("{}")
        
    emit("done", adapter_dir=args.out_dir, adapter_manifest_hash_hex="deadbeef"*8)

if __name__ == "__main__":
    main()
