import blake3

def deterministic_chunk(text: str, source_id: str, max_size: int = 1000) -> list:
    """
    Chunks text deterministically.
    Returns list of dicts: {'id': ..., 'text': ..., 'offset': ...}
    """
    chunks = []
    current_offset = 0
    
    # Very simple chunking for MVP: split by double newline or fixed size
    # A real implementation would use a better strategy (e.g. semantic or sentence based)
    
    paragraphs = text.split("\n\n")
    current_chunk = ""
    chunk_start_offset = 0
    
    for para in paragraphs:
        if len(current_chunk) + len(para) < max_size:
            if current_chunk:
                current_chunk += "\n\n"
            current_chunk += para
        else:
            if current_chunk:
                chunk_id = blake3.blake3(f"{source_id}:{chunk_start_offset}".encode()).hexdigest()
                chunks.append({
                    "id": chunk_id,
                    "text": current_chunk,
                    "offset": chunk_start_offset
                })
            
            chunk_start_offset = current_offset + len(current_chunk) # Approx, needs careful handling for exact byte offsets if critical
            # Reset
            current_chunk = para
            
        current_offset += len(para) + 2 # +2 for \n\n

    # Last chunk
    if current_chunk:
        chunk_id = blake3.blake3(f"{source_id}:{chunk_start_offset}".encode()).hexdigest()
        chunks.append({
            "id": chunk_id,
            "text": current_chunk,
            "offset": chunk_start_offset
        })
        
    return chunks
