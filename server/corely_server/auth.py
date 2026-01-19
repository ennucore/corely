"""Authentication and authorization for Corely server."""

import os
import secrets
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Optional

import bcrypt
from fastapi import Depends, HTTPException, status
from fastapi.security import HTTPBearer, HTTPAuthorizationCredentials
from jose import JWTError, jwt
from pydantic import BaseModel

# Configuration
ALGORITHM = "HS256"
ACCESS_TOKEN_EXPIRE_MINUTES = 60 * 24 * 7  # 7 days for better UX
WORKER_TOKEN = os.environ.get("CORELY_WORKER_TOKEN", "corely-worker-secret")
PENDING_TOKEN_EXPIRE_MINUTES = 5

# Persistent SECRET_KEY - load from env or file
def _get_secret_key() -> str:
    # Try environment variable first
    if key := os.environ.get("CORELY_SECRET_KEY"):
        return key

    # Try to load from file
    key_file = Path("/opt/corely/secret.key")
    if not key_file.exists():
        key_file = Path.home() / ".config" / "corely" / "secret.key"

    if key_file.exists():
        return key_file.read_text().strip()

    # Generate and save a new key
    key = secrets.token_urlsafe(32)
    key_file.parent.mkdir(parents=True, exist_ok=True)
    key_file.write_text(key)
    key_file.chmod(0o600)
    return key

SECRET_KEY = _get_secret_key()

security = HTTPBearer()

# German day names (0=Monday, 6=Sunday)
GERMAN_DAYS = ["montag", "dienstag", "mittwoch", "donnerstag", "freitag", "samstag", "sonntag"]

# Store pending auth tokens (in production, use Redis or similar)
_pending_tokens: dict[str, dict] = {}

# User database - loaded from storage
_users_db: dict[str, dict] = {}
_users_loaded = False


class Token(BaseModel):
    access_token: str
    token_type: str


class TokenData(BaseModel):
    username: Optional[str] = None
    scopes: list[str] = []


class User(BaseModel):
    username: str
    disabled: bool = False
    scopes: list[str] = ["read", "write", "admin"]


def verify_password(plain_password: str, hashed_password: str) -> bool:
    return bcrypt.checkpw(plain_password.encode(), hashed_password.encode())


def get_password_hash(password: str) -> str:
    return bcrypt.hashpw(password.encode(), bcrypt.gensalt()).decode()


async def _ensure_users_loaded():
    """Load users from database if not already loaded."""
    global _users_db, _users_loaded
    if _users_loaded:
        return

    from . import storage
    users = await storage.get_all_users()
    _users_db = {u["username"]: u for u in users}

    # Create default admin if no users exist
    if not _users_db:
        default_user = {
            "username": "llev",
            "hashed_password": get_password_hash("admin"),
            "disabled": False,
            "scopes": ["read", "write", "admin"],
        }
        await storage.create_user(default_user)
        _users_db["llev"] = default_user

    _users_loaded = True


def _reload_users():
    """Force reload users on next access."""
    global _users_loaded
    _users_loaded = False


async def get_user(username: str) -> Optional[User]:
    await _ensure_users_loaded()
    if username in _users_db:
        user_dict = _users_db[username]
        return User(**user_dict)
    return None


async def authenticate_user(username: str, password: str) -> Optional[User]:
    user = await get_user(username)
    if not user:
        return None
    user_data = _users_db.get(username)
    if not user_data:
        return None
    if not verify_password(password, user_data["hashed_password"]):
        return None
    return user


async def change_password(username: str, new_password: str) -> bool:
    """Change a user's password."""
    from . import storage

    await _ensure_users_loaded()
    if username not in _users_db:
        return False

    hashed = get_password_hash(new_password)
    await storage.update_user_password(username, hashed)
    _users_db[username]["hashed_password"] = hashed
    return True


async def change_username(old_username: str, new_username: str) -> bool:
    """Change a user's username."""
    from . import storage

    await _ensure_users_loaded()
    if old_username not in _users_db:
        return False
    if new_username in _users_db:
        return False  # Username already taken

    await storage.update_username(old_username, new_username)
    _users_db[new_username] = _users_db.pop(old_username)
    _users_db[new_username]["username"] = new_username
    return True


def create_access_token(data: dict, expires_delta: Optional[timedelta] = None) -> str:
    to_encode = data.copy()
    if expires_delta:
        expire = datetime.now(timezone.utc) + expires_delta
    else:
        expire = datetime.now(timezone.utc) + timedelta(minutes=ACCESS_TOKEN_EXPIRE_MINUTES)
    to_encode.update({"exp": expire})
    encoded_jwt = jwt.encode(to_encode, SECRET_KEY, algorithm=ALGORITHM)
    return encoded_jwt


async def get_current_user(
    credentials: HTTPAuthorizationCredentials = Depends(security),
) -> User:
    credentials_exception = HTTPException(
        status_code=status.HTTP_401_UNAUTHORIZED,
        detail="Could not validate credentials",
        headers={"WWW-Authenticate": "Bearer"},
    )
    try:
        token = credentials.credentials
        payload = jwt.decode(token, SECRET_KEY, algorithms=[ALGORITHM])
        username: str = payload.get("sub")
        if username is None:
            raise credentials_exception
        scopes = payload.get("scopes", [])
        token_data = TokenData(username=username, scopes=scopes)
    except JWTError:
        raise credentials_exception

    user = await get_user(token_data.username)
    if user is None:
        raise credentials_exception
    if user.disabled:
        raise HTTPException(status_code=400, detail="Inactive user")
    return user


def verify_worker_token(token: str) -> bool:
    """Verify the pre-shared worker token."""
    return secrets.compare_digest(token, WORKER_TOKEN)


def verify_2fa_code(code: str) -> bool:
    """Verify the 2FA code."""
    now = datetime.now()
    day_of_month = now.day
    # Next day of week (0=Monday, 6=Sunday)
    next_day_index = (now.weekday() + 1) % 7
    next_day_german = GERMAN_DAYS[next_day_index]

    expected_code = f"{day_of_month + 23}{next_day_german}"
    return code.lower().strip() == expected_code


def create_pending_token(username: str) -> str:
    """Create a short-lived token for 2FA verification."""
    token = secrets.token_urlsafe(32)
    expires = datetime.now(timezone.utc) + timedelta(minutes=PENDING_TOKEN_EXPIRE_MINUTES)
    _pending_tokens[token] = {
        "username": username,
        "expires": expires,
    }
    return token


def verify_pending_token(token: str) -> Optional[str]:
    """Verify pending token and return username if valid."""
    if token not in _pending_tokens:
        return None

    data = _pending_tokens[token]
    if datetime.now(timezone.utc) > data["expires"]:
        del _pending_tokens[token]
        return None

    return data["username"]


def consume_pending_token(token: str) -> Optional[str]:
    """Consume pending token (one-time use) and return username."""
    username = verify_pending_token(token)
    if username:
        del _pending_tokens[token]
    return username


def require_scope(required_scope: str):
    """Dependency to require a specific scope."""

    async def check_scope(user: User = Depends(get_current_user)) -> User:
        if required_scope not in user.scopes and "admin" not in user.scopes:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail=f"Scope '{required_scope}' required",
            )
        return user

    return check_scope
