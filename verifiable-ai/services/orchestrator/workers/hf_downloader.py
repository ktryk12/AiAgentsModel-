#!/usr/bin/env python3
import json, sys, time, os
from huggingface_hub import snapshot_download

def emit(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

def main():
    # args: repo_id, revision(optional), allow_patterns(optional)
    if len(sys.argv) < 2:
        emit({"type":"error","message":"usage: hf_downloader.py <repo_id> [revision]"})
        return 2

    repo_id = sys.argv[1]
    revision = sys.argv[2] if len(sys.argv) >= 3 else None

    out_dir = os.environ.get("HF_OUT_DIR", "/app/artifacts/hf")
    os.makedirs(out_dir, exist_ok=True)

    emit({"type":"start","worker":"hf_downloader","repo_id":repo_id,"revision":revision,"out_dir":out_dir})

    t0 = time.time()
    try:
        path = snapshot_download(
            repo_id=repo_id,
            revision=revision,
            local_dir=os.path.join(out_dir, repo_id.replace("/", "__")),
            local_dir_use_symlinks=False,
            # progress is internal; we’ll emit coarse progress around phases
        )
        dt = round(time.time() - t0, 3)
        emit({"type":"done","worker":"hf_downloader","path":path,"seconds":dt})
        return 0
    except Exception as e:
        emit({"type":"error","worker":"hf_downloader","message":str(e)})
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
