"""Data collection module for Corely server.

This module handles:
- Collection configuration management per worker
- WebSocket stream handling for receiving data
- Local cache management (40GB LRU)
- R2 upload with optional encryption
"""

from .models import (
    CollectionConfig,
    ScreenConfig,
    CameraConfig,
    AudioInputConfig,
    AudioOutputConfig,
    InputLoggingConfig,
    DirectorySyncConfig,
    CollectionStatus,
)
from .config_manager import ConfigManager
from .stream_handler import StreamHandler
from .cache_manager import CacheManager
from .r2_uploader import R2Uploader
from .encryption import EncryptionManager

__all__ = [
    "CollectionConfig",
    "ScreenConfig",
    "CameraConfig",
    "AudioInputConfig",
    "AudioOutputConfig",
    "InputLoggingConfig",
    "DirectorySyncConfig",
    "CollectionStatus",
    "ConfigManager",
    "StreamHandler",
    "CacheManager",
    "R2Uploader",
    "EncryptionManager",
]
