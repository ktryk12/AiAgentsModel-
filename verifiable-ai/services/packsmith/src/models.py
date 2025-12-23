from pydantic import BaseModel, Field
from typing import List, Optional, Literal, Dict, Any
from datetime import datetime

# --- Internal Job Models ---

class JobState(BaseModel):
    job_id: str
    status: Literal["pending", "ingesting", "normalizing", "chunking", "signing", "packaging", "done", "failed"]
    created_at: datetime
    updated_at: datetime
    pack_id: Optional[str] = None
    version: Optional[str] = None
    error: Optional[str] = None
    artifact_url: Optional[str] = None
    events: List[Dict[str, Any]] = []

# --- API Request Models ---

class BuildPackRequest(BaseModel):
    pack_id: str
    version: str
    name: str = "Untitled Pack"
    source_type: Literal["upload", "website", "git_repo", "paste"]
    source_value: Optional[str] = None  # URL or pasted text. For upload, use multipart form.
    # Optional metadata
    description: Optional[str] = None
    publisher: Optional[str] = "Anonymous"
    license: Optional[str] = "CC-BY-4.0"

# --- Pack Manifest Schema (pack.json) ---

class PackChunk(BaseModel):
    id: str  # Deterministic Hash
    source: str # filename or url
    offset: int
    text: str
    # possibly embeddings later

class PackManifest(BaseModel):
    pack_id: str
    version: str
    name: str
    description: Optional[str] = None
    publisher: str
    license: str
    created_at: str # ISO8601
    content_hash: str # Blake3 hash of all content
    chunk_count: int
    file_count: int
    signature: str # Ed25519 signature of the canonical JSON of this manifest (excluding signature field itself)
    public_key: str # Verification key
