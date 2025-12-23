import os
import json
import shutil
import zipfile
import blake3
from datetime import datetime
from typing import List
from . import chunk, crypto, jobs
from ..models import PackManifest

def create_pack(job_id: str, pack_id: str, version: str, source_dir: str, metadata: dict, job_manager: jobs.JobManager, storage):
    """
    Core pipeline:
    1. Scan source_dir
    2. Normalize & Chunk
    3. Hash Content
    4. Generate Manifest & Sign
    5. Zip & Upload
    """
    
    work_dir = f"/tmp/{job_id}"
    os.makedirs(work_dir, exist_ok=True)
    
    packs_dir = f"{work_dir}/pack"
    docs_dir = f"{packs_dir}/docs"
    os.makedirs(docs_dir, exist_ok=True)
    
    job_manager.emit_event(job_id, "status", {"status": "normalizing"})
    
    all_chunks = []
    file_hashes = {}
    
    # 1. Process Files
    file_count = 0
    total_size = 0
    
    for root, dirs, files in os.walk(source_dir):
        for file in files:
            file_path = os.path.join(root, file)
            rel_path = os.path.relpath(file_path, source_dir)
            
            # Skip hidden or extensive
            if file.startswith(".") or "__pycache__" in root:
                continue

            try:
                with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
                    content = f.read()
                    
                # Store normalized doc
                dest_path = os.path.join(docs_dir, rel_path)
                os.makedirs(os.path.dirname(dest_path), exist_ok=True)
                with open(dest_path, 'w', encoding='utf-8') as f:
                    f.write(content) # Assuming already normalized or raw text for MVP
                    
                # Chunk
                file_chunks = chunk.deterministic_chunk(content, rel_path)
                all_chunks.extend(file_chunks)
                
                # Hash file
                file_hash = crypto.hash_content(content.encode())
                file_hashes[rel_path] = file_hash
                
                file_count += 1
                total_size += len(content)
                
            except Exception as e:
                print(f"Skipping {file}: {e}")

    job_manager.emit_event(job_id, "progress", {"files": file_count, "chunks": len(all_chunks)})
    
    # 2. Manifest
    content_hash_input = "".join(sorted(file_hashes.values()))
    global_hash = crypto.hash_content(content_hash_input.encode())
    
    # Key management: Load from env or generate
    # For MVP we just use the signer
    signature_hex, public_key_hex = crypto.sign_data(global_hash.encode())

    manifest = PackManifest(
        pack_id=pack_id,
        version=version,
        name=metadata.get("name", pack_id),
        description=metadata.get("description"),
        publisher=metadata.get("publisher", "Anonymous"),
        license=metadata.get("license", "CC-BY-4.0"),
        created_at=datetime.utcnow().isoformat(),
        content_hash=global_hash,
        chunk_count=len(all_chunks),
        file_count=file_count,
        signature=signature_hex,
        public_key=public_key_hex
    )
    
    # Write Manifest
    with open(os.path.join(packs_dir, "pack.json"), "w") as f:
         # Use Pydantic's model_dump_json for standard serialization
         # but we want canonical... pydantic doesn't do canonical 'sort_keys' out of box easily
         # convert to dict, then json dump with sort_keys
         f.write(json.dumps(manifest.model_dump(), sort_keys=True, separators=(',', ':')))

    # Write Chunks
    with open(os.path.join(packs_dir, "chunks.ndjson"), "w") as f:
        for c in all_chunks:
            f.write(json.dumps(c, sort_keys=True) + "\n")
            
    # 3. Zip
    job_manager.emit_event(job_id, "status", {"status": "packaging"})
    zip_path = f"{work_dir}/{pack_id}-{version}.zip"
    
    with zipfile.ZipFile(zip_path, 'w', zipfile.ZIP_DEFLATED) as zipf:
        for root, dirs, files in os.walk(packs_dir):
            for file in files:
                file_path = os.path.join(root, file)
                arcname = os.path.relpath(file_path, packs_dir)
                zipf.write(file_path, arcname)

    # 4. Upload
    minio_path = f"packs/{pack_id}/{version}/pack.zip"
    storage.upload_file(minio_path, zip_path)
    
    # Cleanup
    shutil.rmtree(work_dir)
    
    job_manager.emit_event(job_id, "done", {
        "status": "done",
        "pack_id": pack_id,
        "version": version,
        "artifact_url": minio_path
    })
    
    return minio_path
