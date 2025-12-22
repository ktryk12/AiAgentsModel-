import requests
import time
import sys

BASE_URL = "http://localhost:8080"

def log(msg):
    print(f"[TEST] {msg}")

def create_job(kind="lora_train", payload=None):
    if payload is None:
        payload = {"model_id": "test-model"}
    resp = requests.post(f"{BASE_URL}/training/jobs", json={"kind": kind, "payload": payload, "queue": "default"})
    resp.raise_for_status()
    return resp.json()["job_id"]

def get_job(jid):
    return requests.get(f"{BASE_URL}/training/jobs/{jid}").json()

def cancel_job(jid):
    return requests.post(f"{BASE_URL}/training/jobs/{jid}/cancel").json()

def retry_job(jid):
    return requests.post(f"{BASE_URL}/training/jobs/{jid}/retry").json()

def pause_job(jid):
    return requests.post(f"{BASE_URL}/training/jobs/{jid}/pause").json()

def resume_job(jid):
    return requests.post(f"{BASE_URL}/training/jobs/{jid}/resume").json()

def run_verification():
    log("Starting Phase 10 Verification...")
    
    # Test 1: Cancel Pending Job
    log("Test 1: Cancel Pending Job")
    # To Ensure it stays pending, we might need to fill slots or be fast.
    # Actually, we can just submit and immediately cancel. Or start a blocker first.
    
    # Start a blocker job (long running)
    blocker_id = create_job("lora_train", {"duration": 60}) # mock payload
    log(f"Created blocker job {blocker_id}")
    time.sleep(1) # let it run
    
    # Create target pending job
    target_id = create_job("lora_train", {"duration": 10})
    log(f"Created pending job {target_id}")
    j = get_job(target_id)
    if j["status"] != "pending":
        # Maybe it started? If we have 2 workers?
        # Default quota is 1 per queue?
        # Check queue metrics?
        pass
        
    # Cancel it
    resp = cancel_job(target_id)
    log(f"Cancel response: {resp}")
    j = get_job(target_id)
    if j["status"] == "cancelled" and j.get("cancel_requested") == True:
        log("SUCCESS: Pending job cancelled immediately.")
    else:
        log(f"FAILURE: Job status is {j['status']}")
        sys.exit(1)

    # Test 2: Cancel Running Job (The blocker)
    log("Test 2: Cancel Running Job")
    j = get_job(blocker_id)
    if j["status"] != "running":
        log(f"WARNING: Blocker job is {j['status']}, expected running. It might have finished or failed.")
    
    cancel_job(blocker_id)
    log(f"Requested cancel for running job {blocker_id}")
    
    # Wait for worker to term
    for _ in range(10):
        time.sleep(1)
        j = get_job(blocker_id)
        if j["status"] == "cancelled":
            log("SUCCESS: Running job transitioned to cancelled.")
            if j.get("finished_at"):
                log("SUCCESS: finished_at is set.")
            break
    else:
        log(f"FAILURE: Running job did not cancel in time. Status: {j['status']}")
        sys.exit(1)

    # Test 3: Retry Job
    log("Test 3: Retry Job")
    resp = retry_job(target_id) # Retry the cancelled pending job
    log(f"Retry response: {resp}")
    j = get_job(target_id)
    if j["status"] == "pending" and j.get("cancel_requested") == False:
        log("SUCCESS: Job retried and is pending.")
    else:
        log(f"FAILURE: Retried job status is {j['status']}")
        sys.exit(1)
        
    log("Phase 10 Verification Complete!")

if __name__ == "__main__":
    try:
        run_verification()
    except Exception as e:
        print(f"ERROR: {e}")
        sys.exit(1)
