"""Pydantic models for data collection configuration."""

from datetime import datetime
from typing import Optional
from pydantic import BaseModel, Field


class ScreenConfig(BaseModel):
    """Screen capture configuration."""
    enabled: bool = False
    fps: int = Field(default=1, ge=1, le=30)
    resolution: int = Field(default=720, ge=240, le=2160)
    all_displays: bool = True
    display_ids: list[int] = Field(default_factory=list)
    quality: int = Field(default=80, ge=1, le=100)


class CameraConfig(BaseModel):
    """Camera capture configuration."""
    enabled: bool = False
    fps: int = Field(default=5, ge=1, le=30)
    resolution: int = Field(default=480, ge=240, le=1080)
    all_cameras: bool = True
    camera_indices: list[int] = Field(default_factory=list)
    implies_mic: bool = True


class AudioInputConfig(BaseModel):
    """Audio input (microphone) configuration."""
    enabled: bool = False
    sample_rate: int = Field(default=44100)
    device: Optional[str] = None


class AudioOutputConfig(BaseModel):
    """Audio output (system loopback) configuration."""
    enabled: bool = False
    sample_rate: int = Field(default=44100)
    device: Optional[str] = None


class InputLoggingConfig(BaseModel):
    """Input logging (keystrokes/mouse) configuration."""
    enabled: bool = False
    log_keystrokes: bool = True
    log_mouse_moves: bool = True
    log_mouse_clicks: bool = True
    mouse_sample_ms: int = Field(default=100, ge=10, le=1000)


class DirectorySyncConfig(BaseModel):
    """Directory sync configuration."""
    paths: list[str] = Field(default_factory=list)
    include_patterns: list[str] = Field(default_factory=list)
    exclude_patterns: list[str] = Field(default_factory=list)
    sync_interval_secs: int = Field(default=300, ge=60, le=86400)
    max_file_size: int = Field(default=100 * 1024 * 1024)  # 100MB
    watch_changes: bool = True


class CollectionConfig(BaseModel):
    """Main collection configuration for a worker."""
    screen: ScreenConfig = Field(default_factory=ScreenConfig)
    camera: CameraConfig = Field(default_factory=CameraConfig)
    audio_input: AudioInputConfig = Field(default_factory=AudioInputConfig)
    audio_output: AudioOutputConfig = Field(default_factory=AudioOutputConfig)
    input_logging: InputLoggingConfig = Field(default_factory=InputLoggingConfig)
    directory_sync: DirectorySyncConfig = Field(default_factory=DirectorySyncConfig)
    chunk_duration_secs: int = Field(default=60, ge=10, le=300)
    output_dir: str = "/tmp/corely_collection"

    def apply_defaults(self):
        """Apply default coupling rules."""
        # Screen on implies audio output on
        if self.screen.enabled and not self.audio_output.enabled:
            self.audio_output.enabled = True

        # Screen on implies input logging on
        if self.screen.enabled and not self.input_logging.enabled:
            self.input_logging.enabled = True

        # Camera on with implies_mic implies audio input on
        if self.camera.enabled and self.camera.implies_mic and not self.audio_input.enabled:
            self.audio_input.enabled = True

        return self


class CollectionStatus(BaseModel):
    """Current collection status for a worker."""
    is_collecting: bool = False
    session_id: Optional[str] = None
    started_at: Optional[datetime] = None
    ended_at: Optional[datetime] = None
    chunk_count: int = 0
    active_streams: list[str] = Field(default_factory=list)
    last_error: Optional[str] = None


class CollectionSession(BaseModel):
    """A collection session record."""
    session_id: str
    worker_id: str
    started_at: Optional[datetime] = None
    ended_at: Optional[datetime] = None
    status: str = "active"
    total_chunks: int = 0


class CollectionChunk(BaseModel):
    """A chunk of collected data."""
    chunk_id: str
    session_id: str
    worker_id: str
    chunk_index: int
    start_timestamp: Optional[int] = None
    end_timestamp: Optional[int] = None
    local_path: Optional[str] = None
    r2_path: Optional[str] = None
    size_bytes: Optional[int] = None
    encrypted: bool = False
    status: str = "recording"


class R2ConfigRequest(BaseModel):
    """Request to set R2 configuration."""
    endpoint: str
    access_key: str
    secret_key: str
    bucket_normal: str
    bucket_infrequent: Optional[str] = None


class EncryptionKeyRequest(BaseModel):
    """Request to set encryption password."""
    password: str


class StreamFrame(BaseModel):
    """A frame of streaming data."""
    chunk_index: int
    timestamp_ms: int
    data_type: int  # 0=video, 1=mic, 2=output, 3=input, 4=file
    data: bytes

    class Config:
        arbitrary_types_allowed = True
