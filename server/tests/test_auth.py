"""Tests for authentication module."""

import pytest
from datetime import timedelta

from corely_server.auth import (
    verify_password,
    get_password_hash,
    authenticate_user,
    create_access_token,
    verify_worker_token,
    WORKER_TOKEN,
)


class TestPasswordHashing:
    """Tests for password hashing functions."""

    def test_hash_and_verify_password(self):
        """Test that password hashing and verification works."""
        password = "test_password_123"
        hashed = get_password_hash(password)

        assert hashed != password
        assert verify_password(password, hashed)

    def test_wrong_password_fails(self):
        """Test that wrong password fails verification."""
        password = "correct_password"
        wrong_password = "wrong_password"
        hashed = get_password_hash(password)

        assert not verify_password(wrong_password, hashed)

    def test_different_hashes_for_same_password(self):
        """Test that same password produces different hashes (salting)."""
        password = "same_password"
        hash1 = get_password_hash(password)
        hash2 = get_password_hash(password)

        assert hash1 != hash2
        assert verify_password(password, hash1)
        assert verify_password(password, hash2)


class TestUserAuthentication:
    """Tests for user authentication."""

    def test_authenticate_valid_user(self):
        """Test authentication with valid credentials."""
        user = authenticate_user("admin", "admin")
        assert user is not None
        assert user.username == "admin"

    def test_authenticate_invalid_username(self):
        """Test authentication with invalid username."""
        user = authenticate_user("nonexistent", "password")
        assert user is None

    def test_authenticate_invalid_password(self):
        """Test authentication with invalid password."""
        user = authenticate_user("admin", "wrong_password")
        assert user is None


class TestTokenCreation:
    """Tests for JWT token creation."""

    def test_create_token_with_expiry(self):
        """Test creating a token with custom expiry."""
        data = {"sub": "testuser", "scopes": ["read"]}
        token = create_access_token(data, expires_delta=timedelta(hours=1))

        assert isinstance(token, str)
        assert len(token) > 0

    def test_create_token_default_expiry(self):
        """Test creating a token with default expiry."""
        data = {"sub": "testuser"}
        token = create_access_token(data)

        assert isinstance(token, str)
        assert len(token) > 0


class TestWorkerToken:
    """Tests for worker token verification."""

    def test_verify_valid_worker_token(self):
        """Test verification with valid worker token."""
        assert verify_worker_token(WORKER_TOKEN)

    def test_verify_invalid_worker_token(self):
        """Test verification with invalid worker token."""
        assert not verify_worker_token("wrong_token")
        assert not verify_worker_token("")
        assert not verify_worker_token("almost-" + WORKER_TOKEN)
