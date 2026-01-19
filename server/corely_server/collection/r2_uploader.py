"""R2 (S3-compatible) uploader for collected data."""

import asyncio
import os
from pathlib import Path
from typing import Optional
from datetime import datetime
import hashlib
import shutil
import tempfile

import boto3
from botocore.config import Config

from ..storage import storage
from .cache_manager import cache_manager
from .encryption import encryption_manager


class R2Uploader:
    """Handles uploading chunks to Cloudflare R2 storage."""

    def __init__(self):
        self._client = None
        self._config: Optional[dict] = None
        self._upload_task: Optional[asyncio.Task] = None
        self._running = False

    async def init(self):
        """Initialize the R2 client from stored config."""
        config = await storage.get_r2_config()
        if config:
            await self._configure_client(config)

    async def configure(
        self,
        endpoint: str,
        access_key: str,
        secret_key: str,
        bucket_normal: str,
        bucket_infrequent: Optional[str] = None,
    ):
        """Configure R2 credentials.

        Note: Credentials are encrypted before storage.
        """
        # Simple encryption for credentials (in production, use proper key management)
        from cryptography.fernet import Fernet
        key = os.environ.get("CORELY_ENCRYPTION_KEY", Fernet.generate_key())
        if isinstance(key, str):
            key = key.encode()
        fernet = Fernet(key)

        access_key_encrypted = fernet.encrypt(access_key.encode()).decode()
        secret_key_encrypted = fernet.encrypt(secret_key.encode()).decode()

        await storage.set_r2_config(
            endpoint=endpoint,
            access_key_encrypted=access_key_encrypted,
            secret_key_encrypted=secret_key_encrypted,
            bucket_normal=bucket_normal,
            bucket_infrequent=bucket_infrequent,
        )

        await self._configure_client({
            "endpoint": endpoint,
            "access_key_encrypted": access_key_encrypted,
            "secret_key_encrypted": secret_key_encrypted,
            "bucket_normal": bucket_normal,
            "bucket_infrequent": bucket_infrequent,
        })

    async def _configure_client(self, config: dict):
        """Configure the S3 client."""
        self._config = config

        # Decrypt credentials
        from cryptography.fernet import Fernet
        key = os.environ.get("CORELY_ENCRYPTION_KEY", "").encode()
        if key:
            try:
                fernet = Fernet(key)
                access_key = fernet.decrypt(config["access_key_encrypted"].encode()).decode()
                secret_key = fernet.decrypt(config["secret_key_encrypted"].encode()).decode()
            except Exception:
                # Fallback: assume plaintext (development mode)
                access_key = config["access_key_encrypted"]
                secret_key = config["secret_key_encrypted"]
        else:
            access_key = config["access_key_encrypted"]
            secret_key = config["secret_key_encrypted"]

        self._client = boto3.client(
            "s3",
            endpoint_url=config["endpoint"],
            aws_access_key_id=access_key,
            aws_secret_access_key=secret_key,
            config=Config(
                signature_version="s3v4",
                retries={"max_attempts": 3, "mode": "standard"},
            ),
        )

    def is_configured(self) -> bool:
        """Check if R2 is configured."""
        return self._client is not None

    async def start_background_upload(self):
        """Start background upload task."""
        if self._upload_task is not None:
            return

        self._running = True
        self._upload_task = asyncio.create_task(self._upload_loop())

    async def stop_background_upload(self):
        """Stop background upload task."""
        self._running = False
        if self._upload_task:
            self._upload_task.cancel()
            try:
                await self._upload_task
            except asyncio.CancelledError:
                pass
            self._upload_task = None

    async def _upload_loop(self):
        """Background loop to upload completed chunks."""
        while self._running:
            try:
                # Get chunks ready for upload
                chunks = await storage.get_chunks_to_upload(limit=5)

                for chunk in chunks:
                    if not self._running:
                        break

                    try:
                        await self._upload_chunk(chunk)
                    except Exception as e:
                        # Log error but continue with other chunks
                        print(f"Failed to upload chunk {chunk['chunk_id']}: {e}")

                # Wait before checking again
                await asyncio.sleep(10)

            except asyncio.CancelledError:
                break
            except Exception as e:
                print(f"Upload loop error: {e}")
                await asyncio.sleep(30)

    async def _upload_chunk(self, chunk: dict):
        """Upload a single chunk to R2."""
        if not self.is_configured():
            return

        chunk_id = chunk["chunk_id"]
        local_path = chunk.get("local_path")
        worker_id = chunk["worker_id"]

        if not local_path or not Path(local_path).exists():
            return

        # Determine bucket (normal vs infrequent access)
        config_record = await storage.get_collection_config(worker_id)
        use_ia = config_record and config_record.get("use_infrequent_access", False)

        bucket = (
            self._config["bucket_infrequent"]
            if use_ia and self._config.get("bucket_infrequent")
            else self._config["bucket_normal"]
        )

        # Check if encryption is needed
        encryption_key = config_record.get("encryption_public_key") if config_record else None

        # Create archive of chunk directory
        with tempfile.NamedTemporaryFile(suffix=".tar.gz", delete=False) as tmp:
            tmp_path = tmp.name

        try:
            # Create tar.gz archive
            shutil.make_archive(
                tmp_path.replace(".tar.gz", ""),
                "gztar",
                local_path,
            )

            upload_path = tmp_path

            # Encrypt if key is set
            encrypted = False
            if encryption_key:
                try:
                    salt, public_key = encryption_manager.decode_key_from_storage(encryption_key)
                    encrypted_path = tmp_path + ".enc"
                    encryption_manager.encrypt_file(tmp_path, encrypted_path, public_key)
                    upload_path = encrypted_path
                    encrypted = True
                except Exception as e:
                    print(f"Encryption failed for chunk {chunk_id}: {e}")

            # Generate R2 path
            r2_path = f"{worker_id}/{chunk['session_id']}/{chunk_id}"
            if encrypted:
                r2_path += ".enc"
            else:
                r2_path += ".tar.gz"

            # Upload to R2
            with open(upload_path, "rb") as f:
                self._client.upload_fileobj(
                    f,
                    bucket,
                    r2_path,
                    ExtraArgs={
                        "ContentType": "application/octet-stream",
                        "Metadata": {
                            "chunk_id": chunk_id,
                            "worker_id": worker_id,
                            "encrypted": str(encrypted).lower(),
                        },
                    },
                )

            # Update database
            await storage.mark_chunk_uploaded(chunk_id, f"{bucket}/{r2_path}", encrypted)

            # Clean up temp files
            os.unlink(tmp_path)
            if encrypted and os.path.exists(upload_path):
                os.unlink(upload_path)

        except Exception as e:
            # Clean up on error
            if os.path.exists(tmp_path):
                os.unlink(tmp_path)
            raise

    async def download_chunk(self, chunk: dict, output_dir: str, password: Optional[str] = None) -> str:
        """Download a chunk from R2.

        Args:
            chunk: Chunk record from database
            output_dir: Directory to download to
            password: Password for decryption (if encrypted)

        Returns:
            Path to downloaded chunk directory
        """
        if not self.is_configured():
            raise ValueError("R2 not configured")

        r2_path = chunk.get("r2_path")
        if not r2_path:
            raise ValueError("Chunk not uploaded to R2")

        # Parse bucket and key from r2_path
        bucket, key = r2_path.split("/", 1)

        # Download to temp file
        with tempfile.NamedTemporaryFile(suffix=".tar.gz", delete=False) as tmp:
            tmp_path = tmp.name

        try:
            self._client.download_file(bucket, key, tmp_path)

            # Decrypt if needed
            if chunk.get("encrypted") and password:
                config_record = await storage.get_collection_config(chunk["worker_id"])
                encryption_key = config_record.get("encryption_public_key") if config_record else None

                if encryption_key:
                    salt, _ = encryption_manager.decode_key_from_storage(encryption_key)
                    _, private_key, _ = encryption_manager.derive_keypair(password, salt)

                    decrypted_path = tmp_path.replace(".enc", "")
                    encryption_manager.decrypt_file(tmp_path, decrypted_path, private_key)
                    os.unlink(tmp_path)
                    tmp_path = decrypted_path

            # Extract archive
            output_path = Path(output_dir) / chunk["chunk_id"]
            shutil.unpack_archive(tmp_path, output_path)

            os.unlink(tmp_path)
            return str(output_path)

        except Exception as e:
            if os.path.exists(tmp_path):
                os.unlink(tmp_path)
            raise

    async def get_presigned_url(self, r2_path: str, expires_in: int = 3600) -> str:
        """Generate a presigned URL for direct download.

        Args:
            r2_path: Path in format "bucket/key"
            expires_in: URL expiration time in seconds

        Returns:
            Presigned URL
        """
        if not self.is_configured():
            raise ValueError("R2 not configured")

        bucket, key = r2_path.split("/", 1)

        return self._client.generate_presigned_url(
            "get_object",
            Params={"Bucket": bucket, "Key": key},
            ExpiresIn=expires_in,
        )


# Global instance
r2_uploader = R2Uploader()
