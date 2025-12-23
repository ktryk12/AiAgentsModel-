import json
import os
import glob
from typing import List, Optional
from datetime import datetime
from ..models import JobState

JOB_DIR = "/data/jobs" # Inside container
if not os.path.exists(JOB_DIR):
    os.makedirs(JOB_DIR, exist_ok=True)

class JobManager:
    def create_job(self) -> str:
        job_id = f"job-{datetime.now().strftime('%Y%m%d%H%M%S')}-{os.urandom(4).hex()}"
        job_path = os.path.join(JOB_DIR, f"{job_id}.ndjson")
        
        # Create empty file
        with open(job_path, 'w') as f:
            pass
            
        self.emit_event(job_id, "created", {"status": "pending"})
        return job_id

    def emit_event(self, job_id: str, type: str, data: dict):
        unique_timestamp = datetime.now().isoformat()
        event = {
            "timestamp": unique_timestamp,
            "type": type,
            **data
        }
        
        job_path = os.path.join(JOB_DIR, f"{job_id}.ndjson")
        with open(job_path, 'a') as f:
            f.write(json.dumps(event) + "\n")

    def get_job(self, job_id: str) -> Optional[JobState]:
        job_path = os.path.join(JOB_DIR, f"{job_id}.ndjson")
        if not os.path.exists(job_path):
            return None

        # Replay events
        state = JobState(
            job_id=job_id, 
            status="pending", 
            created_at=datetime.now(), # Estimate
            updated_at=datetime.now()
        )
        
        with open(job_path, 'r') as f:
            for line in f:
                if not line.strip(): continue
                try:
                    evt = json.loads(line)
                    state.events.append(evt)
                    
                    # Apply mutations
                    if evt.get("type") == "created":
                        state.created_at = datetime.fromisoformat(evt["timestamp"])
                    
                    if "status" in evt:
                        state.status = evt["status"]
                        
                    if "pack_id" in evt: state.pack_id = evt["pack_id"]
                    if "version" in evt: state.version = evt["version"]
                    if "error" in evt: state.error = evt["error"]
                    if "artifact_url" in evt: state.artifact_url = evt["artifact_url"]
                    
                    state.updated_at = datetime.fromisoformat(evt["timestamp"])
                except:
                    pass
                    
        return state
