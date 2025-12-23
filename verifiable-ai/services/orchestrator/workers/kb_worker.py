import argparse
import os
import sys
import json
import psycopg2
import time
import random

# Optional: sentence-transformers for local embeddings
try:
    from sentence_transformers import SentenceTransformer
    HAS_LOCAL_EMBEDDING = True
except ImportError:
    HAS_LOCAL_EMBEDDING = False

# DB connection string
DB_URL = os.environ.get("DATABASE_URL", "postgres://verifiable:verifiable@postgres:5432/verifiable_ai")

def get_db():
    # Retry logic for startup
    for i in range(5):
        try:
            return psycopg2.connect(DB_URL)
        except Exception:
            time.sleep(2)
    return psycopg2.connect(DB_URL)

def get_embedding_model():
    if HAS_LOCAL_EMBEDDING:
        return SentenceTransformer('all-MiniLM-L6-v2')
    return None

def embed_text(model, text):
    if model:
        return model.encode(text).tolist()
    # Fallback: Random vector (for dev/testing when lib is missing)
    # in real usage, we should call an API here.
    return [0.0] * 384 

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-type", required=True)
    parser.add_argument("--source-path", required=True)
    parser.add_argument("--collection", default="default")
    args = parser.parse_args()

    print(json.dumps({
        "type": "progress",
        "message": f"Loading embedding model (Local={HAS_LOCAL_EMBEDDING})..."
    }))
    sys.stdout.flush()

    # Load model
    model = get_embedding_model()

    print(json.dumps({
        "type": "progress",
        "message": f"Processing source: {args.source_path} ({args.source_type})"
    }))
    sys.stdout.flush()

    documents = []
    
    if args.source_type == "files":
        if os.path.isdir(args.source_path):
             for root, dirs, files in os.walk(args.source_path):
                 for file in files:
                     if file.endswith((".txt", ".md", ".py", ".json")):
                         path = os.path.join(root, file)
                         try:
                             with open(path, 'r', encoding='utf-8', errors='ignore') as f:
                                 documents.append((path, f.read()))
                         except Exception as e:
                             print(json.dumps({"type":"progress", "message":f"Skipping {file}: {e}"}))
        elif os.path.exists(args.source_path):
             with open(args.source_path, 'r', encoding='utf-8', errors='ignore') as f:
                 documents.append((args.source_path, f.read()))
        else:
             documents.append(("raw-input", args.source_path))

    elif args.source_type == "website":
         documents.append((args.source_path, f"Placeholder content for website: {args.source_path}"))
    else:
         documents.append(("unknown", str(args.source_path)))

    if not documents:
        print(json.dumps({"type":"error", "message":"No documents found to ingest."}))
        return

    # Chunking
    chunks = []
    CHUNK_SIZE = 500
    
    for src, text in documents:
        content_len = len(text)
        for i in range(0, content_len, CHUNK_SIZE):
            chunk_text = text[i:i+CHUNK_SIZE]
            chunks.append((src, chunk_text))

    print(json.dumps({
        "type": "progress",
        "message": f"Generated {len(chunks)} chunks. Embedding..."
    }))
    sys.stdout.flush()

    # Embedding
    # texts = [c[1] for c in chunks]
    # embeddings = model.encode(texts) # if local

    # Storing
    conn = get_db()
    cur = conn.cursor()
    
    try:
        count = 0
        for i, (src, text) in enumerate(chunks):
            # embedding = embeddings[i].tolist()
            embedding = embed_text(model, text)
            
            cur.execute(
                """
                INSERT INTO documents (collection, source, content, embedding)
                VALUES (%s, %s, %s, %s)
                """,
                (args.collection, src, text, embedding)
            )
            count += 1
        conn.commit()
        
        print(json.dumps({
            "type": "result",
            "result": f"Ingested {count} chunks into collection '{args.collection}'"
        }))
        
    except Exception as e:
        conn.rollback()
        print(json.dumps({
            "type": "error",
            "message": str(e)
        }))
        sys.exit(1)
    finally:
        cur.close()
        conn.close()

if __name__ == "__main__":
    main()
