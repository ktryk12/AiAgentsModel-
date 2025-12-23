from minio import Minio
import os
from typing import BinaryIO

class Storage:
    def __init__(self):
        self.endpoint = os.getenv("MINIO_ENDPOINT", "minio:9000")
        self.access_key = os.getenv("MINIO_ACCESS_KEY", "minioadmin")
        self.secret_key = os.getenv("MINIO_SECRET_KEY", "minioadmin")
        self.bucket = os.getenv("MINIO_BUCKET", "verifiable-ai")
        self.secure = os.getenv("MINIO_SECURE", "false").lower() == "true"
        
        # Strip protocol if present
        if "://" in self.endpoint:
             self.endpoint = self.endpoint.split("://")[1]

        self.client = Minio(
            self.endpoint,
            access_key=self.access_key,
            secret_key=self.secret_key,
            secure=self.secure
        )
        
        self._ensure_bucket()

    def _ensure_bucket(self):
        try:
            if not self.client.bucket_exists(self.bucket):
                self.client.make_bucket(self.bucket)
        except Exception as e:
            print(f"Error checking bucket: {e}")

    def upload_file(self, object_name: str, file_path: str):
        self.client.fput_object(self.bucket, object_name, file_path)

    def upload_stream(self, object_name: str, data: BinaryIO, length: int):
        self.client.put_object(self.bucket, object_name, data, length)
