import blake3
import os
from cryptography.hazmat.primitives.asymmetric import ed25519
from cryptography.hazmat.primitives import serialization
from typing import Tuple

def hash_content(data: bytes) -> str:
    """Returns the Blake3 hash of the data."""
    return blake3.blake3(data).hexdigest()

def sign_data(data: bytes, private_key_hex: str = None) -> Tuple[str, str]:
    """
    Signs data using Ed25519.
    Returns (signature_hex, public_key_hex).
    
    If private_key_hex is None, it tries to load from env PACK_SIGNING_KEY.
    If that is missing, it generates a new ephemeral key (WARN: for testing only).
    """
    if private_key_hex is None:
        private_key_hex = os.getenv("PACK_SIGNING_KEY")
    
    if private_key_hex:
        try:
            # Try loading as hex
            private_bytes = bytes.fromhex(private_key_hex)
            private_key = ed25519.Ed25519PrivateKey.from_private_bytes(private_bytes)
        except:
             # If exact 32 bytes provided directly (unlikely in env var but possible)
             private_key = ed25519.Ed25519PrivateKey.from_private_bytes(private_key_hex.encode()[:32])
    else:
        # Fallback for testing/dev if no key provided
        print("WARNING: No PACK_SIGNING_KEY found. Generating ephemeral key.")
        private_key = ed25519.Ed25519PrivateKey.generate()

    public_key = private_key.public_key()
    
    signature = private_key.sign(data)
    
    # Export public key to raw bytes -> hex
    public_bytes = public_key.public_bytes(
        encoding=serialization.Encoding.Raw,
        format=serialization.PublicFormat.Raw
    )
    
    return signature.hex(), public_bytes.hex()
