import argparse
import os
import sys
import json
import requests

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--api-url", required=True, help="LLM API Endpoint")
    parser.add_argument("--model", default="local", help="Model name")
    parser.add_argument("--prompt", required=True, help="User prompt")
    parser.add_argument("--system", default="You are a helpful assistant.", help="System prompt")
    parser.add_argument("--temperature", type=float, default=0.7)
    args = parser.parse_args()

    # Log start
    print(json.dumps({
        "type": "progress",
        "message": f"Generating text with model {args.model} at {args.api_url}..."
    }))
    sys.stdout.flush()

    try:
        payload = {
            "model": args.model,
            "messages": [
                {"role": "system", "content": args.system},
                {"role": "user", "content": args.prompt}
            ],
            "temperature": args.temperature,
            "stream": False
        }

        resp = requests.post(args.api_url, json=payload, timeout=300)
        resp.raise_for_status()
        
        data = resp.json()
        content = data.get("choices", [{}])[0].get("message", {}).get("content", "")

        # Output final result
        print(json.dumps({
            "type": "result",
            "result": content
        }))
    except Exception as e:
        print(json.dumps({
            "type": "error",
            "message": str(e)
        }))
        sys.exit(1)

if __name__ == "__main__":
    main()
