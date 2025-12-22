#!/usr/bin/env python3
import json, sys, time

def emit(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

def main():
    # args: job_id
    job_id = sys.argv[1] if len(sys.argv) >= 2 else "unknown"
    emit({"type":"start","worker":"lora_trainer","job_id":job_id})

    # Simulated training progress
    for i in range(1, 11):
        time.sleep(1)
        emit({"type":"progress","worker":"lora_trainer","job_id":job_id,"pct":i*10,"message":f"step {i}/10"})

    emit({"type":"done","worker":"lora_trainer","job_id":job_id,"artifact":"simulated_adapter.safetensors"})
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
