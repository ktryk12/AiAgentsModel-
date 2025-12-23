from fastapi import FastAPI, BackgroundTasks, UploadFile, File, Form, HTTPException
from fastapi.responses import JSONResponse
import shutil
import os
import uuid
import asyncio

from .models import BuildPackRequest, JobState
from .core import jobs, ingest, pack, storage

app = FastAPI(title="Packsmith", version="0.1.0")

job_manager = jobs.JobManager()
store = storage.Storage()

@app.get("/health")
def health():
    return {"ok": True}

@app.get("/packs")
def list_packs():
    # MVP: Just listing jobs that are done, or query MinIO
    # For simplicity, returning empty list or implementing MinIO list later
    return []

@app.post("/packs/build")
async def build_pack(
    background_tasks: BackgroundTasks,
    pack_id: str = Form(...),
    version: str = Form(...),
    source_type: str = Form(...),
    source_value: str = Form(None),
    name: str = Form(None),
    description: str = Form(None),
    publisher: str = Form(None),
    license: str = Form(None),
    file: UploadFile = File(None)
):
    # Create Job
    job_id = job_manager.create_job()
    
    # Metadata dict
    meta = {
        "name": name,
        "description": description,
        "publisher": publisher,
        "license": license
    }
    
    # Trigger background task
    background_tasks.add_task(
        run_build_job, 
        job_id, 
        pack_id, 
        version, 
        source_type, 
        source_value, 
        file, 
        meta
    )
    
    return {"job_id": job_id, "status": "pending"}

@app.get("/packs/build/{job_id}")
def get_build_status(job_id: str):
    job = job_manager.get_job(job_id)
    if not job:
        raise HTTPException(status_code=404, detail="Job not found")
    return job

async def run_build_job(job_id: str, pack_id: str, version: str, source_type: str, source_value: str, upload_file: UploadFile, meta: dict):
    try:
        job_manager.emit_event(job_id, "status", {"status": "ingesting", "pack_id": pack_id, "version": version})
        
        # 1. Ingest
        source_dir = await ingest.ingest_source(job_id, source_type, source_value, upload_file)
        
        # 2. Build Pack (Normalize -> Chunk -> Sign -> Zip -> Upload)
        # We run this in thread pool to avoid blocking async loop if it does heavy CPU work
        # But pack.create_pack is sync, so wrap it
        loop = asyncio.get_running_loop()
        artifact_url = await loop.run_in_executor(
            None, 
            pack.create_pack, 
            job_id, pack_id, version, source_dir, meta, job_manager, store
        )
        
        # Cleanup source
        shutil.rmtree(source_dir, ignore_errors=True)
        
    except Exception as e:
        import traceback
        traceback.print_exc()
        job_manager.emit_event(job_id, "error", {"error": str(e), "status": "failed"})
