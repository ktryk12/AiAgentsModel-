#!/usr/bin/env python3
import json, sys, time

def emit(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

def main():
    # args: job_id
    job_id = sys.argv[1] if len(sys.argv) >= 2 else "unknown"
    emit({"type":"start","worker":"lora_trainer","job_id":job_id})

    # Simulated training progress (longer duration for crash testing)
    for i in range(1, 61):
        time.sleep(1)
        if i % 10 == 0:
            emit({"type":"progress","worker":"lora_trainer","job_id":job_id,"pct":int((i/60)*100),"message":f"step {i}/60"})

    emit({"type":"done","worker":"lora_trainer","job_id":job_id,"artifact":"simulated_adapter.safetensors"})
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
