-- Enable pgvector extension
CREATE EXTENSION IF NOT EXISTS vector;

-- Table for trusted documents
CREATE TABLE IF NOT EXISTS documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collection VARCHAR(255) NOT NULL, -- e.g. "acme-docs" or "default"
    source VARCHAR(1024) NOT NULL,    -- filepath or URL
    content TEXT NOT NULL,
    metadata JSONB DEFAULT '{}'::jsonb,
    embedding VECTOR(384),            -- 384 dim for all-MiniLM-L6-v2
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Index for IVFFlat (approximate nearest neighbor)
-- Note: 'lists' depends on data size. 100 is okay for small datasets.
-- We create it but it might fail if table is empty, so we use CREATE INDEX IF NOT EXISTS logic safely,
-- but pgvector usually allows creating the index early.
CREATE INDEX IF NOT EXISTS documents_embedding_idx ON documents USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);
