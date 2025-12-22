#!/usr/bin/env python3
import os
import sys
import json
import time
import signal
import argparse
from pathlib import Path

# Clean SIGTERM handling for Phase 10 (Process Control)
def handle_sigterm(sig, frame):
    print(json.dumps({"type": "cancelled", "source": "worker", "message": "Caught SIGTERM, exiting"}), flush=True)
    sys.exit(0)

signal.signal(signal.SIGTERM, handle_sigterm)

def emit(obj):
    sys.stdout.write(json.dumps(obj, ensure_ascii=False) + "\n")
    sys.stdout.flush()

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo_id", required=True)
    ap.add_argument("--revision", default=None)
    ap.add_argument("--cache_dir", default=None)
    ap.add_argument("--local_dir", default=None)
    ap.add_argument("--allow_patterns", default=None, help="Comma-separated patterns")
    ap.add_argument("--ignore_patterns", default=None, help="Comma-separated patterns")
    ap.add_argument("--heartbeat_secs", type=float, default=1.0)
    args = ap.parse_args()

    try:
        from huggingface_hub import snapshot_download
    except Exception as e:
        emit({"type":"error","message":f"Missing dependency huggingface_hub: {e}"})
        return 2

    token = os.environ.get("HF_TOKEN") or None

    allow_patterns = args.allow_patterns.split(",") if args.allow_patterns else None
    ignore_patterns = args.ignore_patterns.split(",") if args.ignore_patterns else None

    emit({
        "type": "start",
        "repo_id": args.repo_id,
        "revision": args.revision,
        "allow_patterns": allow_patterns,
        "ignore_patterns": ignore_patterns,
        "ts": time.time(),
    })

    # Heartbeat threadless: vi printer progress events undervejs ved at poll’e tid
    last_beat = time.time()

    try:
        # snapshot_download er typisk ret “silent”; vi bruger heartbeat mens den arbejder
        # ved at køre den i samme tråd og sende beats før/efter. V1 = coarse.
        emit({"type":"progress","phase":"downloading","detail":"started"})
        snapshot_dir = snapshot_download(
            repo_id=args.repo_id,
            revision=args.revision,
            token=token,
            cache_dir=args.cache_dir,
            local_dir=args.local_dir,
            allow_patterns=allow_patterns,
            ignore_patterns=ignore_patterns,
            resume_download=True,
        )
        emit({"type":"progress","phase":"downloading","detail":"finished"})

        # Enumerér filer for downstream manifest
        p = Path(snapshot_dir)
        files = []
        for f in p.rglob("*"):
            if f.is_file():
                rel = str(f.relative_to(p)).replace("\\", "/")
                files.append({"rel_path": rel, "size": f.stat().st_size})

        # resolved revision kan hentes via repo cache metadata, men den er ikke altid triviel.
        # Vi sender revision input tilbage + snapshot_dir; orchestrator kan resolve senere hvis ønsket.
        emit({
            "type":"done",
            "repo_id": args.repo_id,
            "revision": args.revision,
            "snapshot_dir": str(snapshot_dir),
            "files": files,
            "ts": time.time(),
        })
        return 0

    except Exception as e:
        emit({"type":"error","message":str(e)})
        return 1

if __name__ == "__main__":
    sys.exit(main())
