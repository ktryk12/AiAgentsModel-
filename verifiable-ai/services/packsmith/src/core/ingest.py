import os
import shutil
import zipfile
import httpx
# import git # Requires gitpython or subprocess
import subprocess
from typing import Optional
from fastapi import UploadFile

WORK_ROOT = "/tmp/ingest"

async def ingest_source(job_id: str, source_type: str, source_value: Optional[str] = None, file: Optional[UploadFile] = None) -> str:
    """
    Ingests source into a standardized directory structure.
    Returns path to ingested content.
    """
    dest_dir = os.path.join(WORK_ROOT, job_id)
    if os.path.exists(dest_dir):
        shutil.rmtree(dest_dir)
    os.makedirs(dest_dir, exist_ok=True)

    if source_type == "upload":
        if not file:
            raise ValueError("File required for upload source")
        
        # Save temp file
        temp_zip = os.path.join(dest_dir, "input.zip")
        with open(temp_zip, "wb") as buffer:
            shutil.copyfileobj(file.file, buffer)
            
        # Extract
        with zipfile.ZipFile(temp_zip, 'r') as zip_ref:
            zip_ref.extractall(dest_dir)
            
        os.remove(temp_zip)
        return dest_dir

    elif source_type == "website":
        if not source_value: raise ValueError("URL required")
        # MVP: Single page fetch or simple recursion
        # For now, just a stub implementation that saves 1 page
        async with httpx.AsyncClient() as client:
            resp = await client.get(source_value, follow_redirects=True)
            resp.raise_for_status()
            
            # Save as index.html
            with open(os.path.join(dest_dir, "index.html"), "w", encoding="utf-8") as f:
                f.write(resp.text)
        return dest_dir

    elif source_type == "git_repo":
        if not source_value: raise ValueError("Repo URL required")
        # Shallow clone
        subprocess.run(["git", "clone", "--depth", "1", source_value, dest_dir], check=True)
        # Remove .git
        shutil.rmtree(os.path.join(dest_dir, ".git"), ignore_errors=True)
        return dest_dir

    elif source_type == "paste":
        if not source_value: raise ValueError("Text content required")
        with open(os.path.join(dest_dir, "content.txt"), "w", encoding="utf-8") as f:
            f.write(source_value)
        return dest_dir

    else:
        raise ValueError(f"Unknown source type: {source_type}")
