FROM python:3.11-slim

WORKDIR /app

# Install system dependencies (if any needed for blake3/crypto compilation, though wheels usually exist)
RUN apt-get update && apt-get install -y \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Copy source code
COPY src/ .

# Ensure data directory exists
RUN mkdir -p /data

# Default command
CMD ["uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8000"]
