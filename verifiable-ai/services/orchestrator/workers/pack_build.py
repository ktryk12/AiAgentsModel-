import argparse
import time
import json
import sys
import os
import requests
import shutil
import zipfile

def zip_folder(folder_path, output_path):
    with zipfile.ZipFile(output_path, 'w', zipfile.ZIP_DEFLATED) as zipf:
        for root, dirs, files in os.walk(folder_path):
            for file in files:
                file_path = os.path.join(root, file)
                arcname = os.path.relpath(file_path, folder_path)
                zipf.write(file_path, arcname)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--api-url", default="http://packsmith:8000", help="Packsmith API URL")
    parser.add_argument("--pack-id", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--source-type", default="upload", choices=["upload", "website", "git_repo", "paste"])
    parser.add_argument("--source-value", help="URL or Path or Text")
    parser.add_argument("--name", default="Untitled Pack")
    parser.add_argument("--publisher", default="Anonymous")
    
    args = parser.parse_args()

    # Log Start
    print(json.dumps({
        "type": "status",
        "phase": "starting",
        "message": f"Building pack {args.pack_id} v{args.version}"
    }))
    sys.stdout.flush()

    files = None
    data = {
        "pack_id": args.pack_id,
        "version": args.version,
        "source_type": args.source_type,
        "name": args.name,
        "publisher": args.publisher
    }
    
    temp_zip = None

    try:
        # Handle Local Folder Logic -> Zip -> Upload
        if args.source_type == "upload" and args.source_value and os.path.isdir(args.source_value):
            print(json.dumps({"type": "progress", "message": "Zipping local folder..."}))
            sys.stdout.flush()
            
            temp_zip = f"/tmp/{args.pack_id}-{args.version}.zip"
            zip_folder(args.source_value, temp_zip)
            
            files = {'file': open(temp_zip, 'rb')}
        
        elif args.source_type == "upload" and args.source_value and os.path.isfile(args.source_value):
            files = {'file': open(args.source_value, 'rb')}

        elif args.source_type in ["website", "git_repo", "paste"]:
             # For these, send source_value as form data
             data["source_value"] = args.source_value
        
        # Submit Job
        resp = requests.post(f"{args.api_url}/packs/build", data=data, files=files, timeout=30)
        resp.raise_for_status()
        job_id = resp.json()["job_id"]
        
        print(json.dumps({"type": "status", "phase": "submitted", "job_id": job_id}))
        sys.stdout.flush()
        
        if files:
            files['file'].close()
        if temp_zip and os.path.exists(temp_zip):
            os.remove(temp_zip)

        # Poll Loop
        while True:
            time.sleep(2)
            r = requests.get(f"{args.api_url}/packs/build/{job_id}", timeout=10)
            if r.status_code != 200:
                continue
                
            job_state = r.json()
            status = job_state.get("status")
            
            # Forward events (simplification: just dump state or synthesize events)
            # Ideally we would track last seen event index, but for MVP we just report status
            print(json.dumps({
                "type": "status", 
                "phase": status, 
                "details": job_state.get("events", [])[-1] if job_state.get("events") else {}
            }))
            sys.stdout.flush()
            
            if status == "done":
                print(json.dumps({
                    "type": "done",
                    "pack_id": args.pack_id,
                    "version": args.version,
                    "artifact_url": job_state.get("artifact_url")
                }))
                break
                
            if status == "failed":
                raise Exception(job_state.get("error", "Unknown error"))
                
    except Exception as e:
        print(json.dumps({
            "type": "error",
            "message": str(e)
        }))
        sys.exit(1)

if __name__ == "__main__":
    main()
