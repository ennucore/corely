"""Encryption utilities for data collection.

Uses password-based X25519 keypair derivation for end-to-end encryption.
The private key is NEVER stored - only the public key is kept for encryption.
Decryption requires the user to re-enter their password.
"""

import hashlib
import os
import secrets
from typing import Optional, Tuple

# Using cryptography library for X25519 and AES-GCM
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.kdf.pbkdf2 import PBKDF2HMAC
from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey, X25519PublicKey
from cryptography.hazmat.primitives.ciphers.aead import AESGCM
from cryptography.hazmat.backends import default_backend
import base64


# PBKDF2 iterations (OWASP recommendation for 2023+)
PBKDF2_ITERATIONS = 600_000

# Salt for key derivation (stored alongside public key)
SALT_SIZE = 32

# AES-GCM nonce size
NONCE_SIZE = 12


class EncryptionManager:
    """Manages encryption for data collection."""

    def derive_keypair(self, password: str, salt: Optional[bytes] = None) -> Tuple[bytes, bytes, bytes]:
        """Derive an X25519 keypair from a password.

        Args:
            password: User's password
            salt: Optional salt (generated if not provided)

        Returns:
            Tuple of (public_key_bytes, private_key_bytes, salt)
        """
        if salt is None:
            salt = secrets.token_bytes(SALT_SIZE)

        # Derive 32 bytes for X25519 seed
        kdf = PBKDF2HMAC(
            algorithm=hashes.SHA256(),
            length=32,
            salt=salt,
            iterations=PBKDF2_ITERATIONS,
            backend=default_backend(),
        )
        seed = kdf.derive(password.encode())

        # Create X25519 keypair from seed
        private_key = X25519PrivateKey.from_private_bytes(seed)
        public_key = private_key.public_key()

        return (
            public_key.public_bytes_raw(),
            private_key.private_bytes_raw(),
            salt,
        )

    def derive_public_key(self, password: str, salt: bytes) -> bytes:
        """Derive only the public key from a password.

        Args:
            password: User's password
            salt: Salt used during initial derivation

        Returns:
            Public key bytes
        """
        public_key, _, _ = self.derive_keypair(password, salt)
        return public_key

    def encrypt_data(self, data: bytes, public_key_bytes: bytes) -> bytes:
        """Encrypt data using X25519 + AES-256-GCM.

        Uses ephemeral key exchange for forward secrecy.

        Args:
            data: Data to encrypt
            public_key_bytes: Recipient's public key

        Returns:
            Encrypted data: ephemeral_public_key (32) + nonce (12) + ciphertext + tag (16)
        """
        # Generate ephemeral keypair
        ephemeral_private = X25519PrivateKey.generate()
        ephemeral_public = ephemeral_private.public_key()

        # Perform key exchange
        recipient_public = X25519PublicKey.from_public_bytes(public_key_bytes)
        shared_secret = ephemeral_private.exchange(recipient_public)

        # Derive AES key from shared secret
        aes_key = hashlib.sha256(shared_secret).digest()

        # Encrypt with AES-256-GCM
        nonce = secrets.token_bytes(NONCE_SIZE)
        aesgcm = AESGCM(aes_key)
        ciphertext = aesgcm.encrypt(nonce, data, None)

        # Return: ephemeral_public || nonce || ciphertext
        return ephemeral_public.public_bytes_raw() + nonce + ciphertext

    def decrypt_data(self, encrypted_data: bytes, private_key_bytes: bytes) -> bytes:
        """Decrypt data using X25519 + AES-256-GCM.

        Args:
            encrypted_data: Data encrypted by encrypt_data()
            private_key_bytes: Recipient's private key

        Returns:
            Decrypted data
        """
        # Parse encrypted data
        ephemeral_public_bytes = encrypted_data[:32]
        nonce = encrypted_data[32:32 + NONCE_SIZE]
        ciphertext = encrypted_data[32 + NONCE_SIZE:]

        # Reconstruct keys
        private_key = X25519PrivateKey.from_private_bytes(private_key_bytes)
        ephemeral_public = X25519PublicKey.from_public_bytes(ephemeral_public_bytes)

        # Perform key exchange
        shared_secret = private_key.exchange(ephemeral_public)

        # Derive AES key
        aes_key = hashlib.sha256(shared_secret).digest()

        # Decrypt
        aesgcm = AESGCM(aes_key)
        return aesgcm.decrypt(nonce, ciphertext, None)

    def encrypt_file(self, input_path: str, output_path: str, public_key_bytes: bytes):
        """Encrypt a file.

        Args:
            input_path: Path to input file
            output_path: Path to output encrypted file
            public_key_bytes: Recipient's public key
        """
        with open(input_path, "rb") as f:
            data = f.read()

        encrypted = self.encrypt_data(data, public_key_bytes)

        with open(output_path, "wb") as f:
            f.write(encrypted)

    def decrypt_file(self, input_path: str, output_path: str, private_key_bytes: bytes):
        """Decrypt a file.

        Args:
            input_path: Path to encrypted file
            output_path: Path to output decrypted file
            private_key_bytes: Recipient's private key
        """
        with open(input_path, "rb") as f:
            encrypted = f.read()

        decrypted = self.decrypt_data(encrypted, private_key_bytes)

        with open(output_path, "wb") as f:
            f.write(decrypted)

    @staticmethod
    def encode_key_for_storage(key_bytes: bytes, salt: bytes) -> str:
        """Encode a key and salt for database storage.

        Format: base64(salt) + ":" + base64(key)
        """
        salt_b64 = base64.b64encode(salt).decode()
        key_b64 = base64.b64encode(key_bytes).decode()
        return f"{salt_b64}:{key_b64}"

    @staticmethod
    def decode_key_from_storage(stored: str) -> Tuple[bytes, bytes]:
        """Decode a key and salt from database storage.

        Returns:
            Tuple of (salt, key_bytes)
        """
        salt_b64, key_b64 = stored.split(":")
        salt = base64.b64decode(salt_b64)
        key_bytes = base64.b64decode(key_b64)
        return salt, key_bytes


# Global instance
encryption_manager = EncryptionManager()
