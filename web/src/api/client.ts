// API client for Corely server

// API base URL - can be configured via environment variable for development
// In production, this should be relative ('/api')
// In development, set VITE_API_URL to the full server URL (e.g., 'http://localhost:8000/api')
const API_BASE = import.meta.env.VITE_API_URL || '/api'

// Get the server base URL (without /api) for install commands
export function getServerBaseUrl(): string {
  if (import.meta.env.VITE_API_URL) {
    // Remove '/api' suffix to get base URL
    return import.meta.env.VITE_API_URL.replace(/\/api$/, '')
  }
  // Default: same origin as web UI
  return window.location.origin
}

interface Worker {
  id: string
  name: string
  hostname: string | null
  os: string | null
  arch: string | null
  capabilities: string[]
  is_online: boolean
  last_seen: string | null
  first_seen?: string
}

interface SystemInfo {
  hostname: string
  os: {
    name: string
    version: string
    kernel_version: string
    arch: string
  }
  cpu: {
    brand: string
    cores: number
    usage_percent: number
  }
  memory: {
    total: number
    used: number
    available: number
    usage_percent: number
  }
  swap: {
    total: number
    used: number
  }
  disks: Array<{
    name: string
    mount_point: string
    total_space: number
    available_space: number
    file_system: string
  }>
  network: Array<{
    name: string
    received: number
    transmitted: number
    mac_address: string
  }>
  uptime: number
}

interface ScreenCapture {
  width: number
  height: number
  format: string
  data: string
}

interface ShellResult {
  stdout: string
  stderr: string
  exit_code: number
}

interface OAuthClient {
  client_id: string
  client_secret?: string
  name: string
  scopes: string[]
  created_at: string
  created_by: string
  last_used: string | null
}

// Collection types
interface ScreenConfig {
  enabled: boolean
  fps: number
  resolution: number
  all_displays: boolean
  display_ids: number[]
  quality: number
}

interface CameraConfig {
  enabled: boolean
  fps: number
  resolution: number
  all_cameras: boolean
  camera_indices: number[]
  implies_mic: boolean
}

interface AudioInputConfig {
  enabled: boolean
  sample_rate: number
  device: string | null
}

interface AudioOutputConfig {
  enabled: boolean
  sample_rate: number
  device: string | null
}

interface InputLoggingConfig {
  enabled: boolean
  log_keystrokes: boolean
  log_mouse_moves: boolean
  log_mouse_clicks: boolean
  mouse_sample_ms: number
}

interface DirectorySyncConfig {
  paths: string[]
  include_patterns: string[]
  exclude_patterns: string[]
  sync_interval_secs: number
  max_file_size: number
  watch_changes: boolean
}

interface CollectionConfig {
  screen: ScreenConfig
  camera: CameraConfig
  audio_input: AudioInputConfig
  audio_output: AudioOutputConfig
  input_logging: InputLoggingConfig
  directory_sync: DirectorySyncConfig
  chunk_duration_secs: number
  output_dir: string
}

interface CollectionStatus {
  is_collecting: boolean
  session_id: string | null
  started_at: string | null
  ended_at: string | null
  chunk_count: number
  active_streams: string[]
  last_error: string | null
}

interface CollectionSession {
  session_id: string
  worker_id: string
  started_at: string | null
  ended_at: string | null
  status: string
  total_chunks: number
}

interface CollectionChunk {
  chunk_id: string
  session_id: string
  worker_id: string
  chunk_index: number
  start_timestamp: number | null
  end_timestamp: number | null
  local_path: string | null
  r2_path: string | null
  size_bytes: number | null
  encrypted: boolean
  status: string
}

interface CacheStats {
  cache_dir: string
  max_size_bytes: number
  current_size_bytes: number
  usage_percent: number
  chunk_count: number
}

class CorelyAPI {
  private token: string | null = null

  setToken(token: string) {
    this.token = token
    localStorage.setItem('corely_token', token)
  }

  getToken(): string | null {
    if (!this.token) {
      this.token = localStorage.getItem('corely_token')
    }
    return this.token
  }

  clearToken() {
    this.token = null
    localStorage.removeItem('corely_token')
  }

  private async fetch<T>(path: string, options: RequestInit = {}): Promise<T> {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
    }

    const token = this.getToken()
    if (token) {
      headers['Authorization'] = `Bearer ${token}`
    }

    const response = await fetch(`${API_BASE}${path}`, {
      ...options,
      headers,
    })

    if (response.status === 401) {
      this.clearToken()
      window.location.href = '/login'
      throw new Error('Unauthorized')
    }

    if (!response.ok) {
      const error = await response.json().catch(() => ({ detail: 'Unknown error' }))
      throw new Error(error.detail || 'Request failed')
    }

    return response.json()
  }

  async login(username: string, password: string): Promise<string> {
    const formData = new URLSearchParams()
    formData.append('username', username)
    formData.append('password', password)

    const response = await fetch(`${API_BASE}/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: formData,
    })

    if (!response.ok) {
      throw new Error('Invalid credentials')
    }

    const data = await response.json()
    this.setToken(data.access_token)
    return data.access_token
  }

  async getMe(): Promise<{ username: string; scopes: string[] }> {
    return this.fetch('/auth/me')
  }

  async listWorkers(): Promise<Worker[]> {
    const data = await this.fetch<{ workers: Worker[] }>('/workers')
    return data.workers
  }

  async getWorker(id: string): Promise<Worker> {
    return this.fetch(`/workers/${id}`)
  }

  async getSystemInfo(workerId: string): Promise<SystemInfo> {
    return this.fetch(`/workers/${workerId}/system`)
  }

  async getScreen(workerId: string, displayId?: number): Promise<ScreenCapture> {
    const params = displayId ? `?display_id=${displayId}` : ''
    return this.fetch(`/workers/${workerId}/screen${params}`)
  }

  async execShell(workerId: string, command: string, timeout = 30000): Promise<ShellResult> {
    return this.fetch(`/workers/${workerId}/shell?command=${encodeURIComponent(command)}&timeout=${timeout}`, {
      method: 'POST',
    })
  }

  async callWorker(workerId: string, method: string, params: Record<string, unknown> = {}): Promise<unknown> {
    const data = await this.fetch<{ result: unknown }>(`/workers/${workerId}/call`, {
      method: 'POST',
      body: JSON.stringify({ method, params }),
    })
    return data.result
  }

  getTerminalWsUrl(workerId: string): string {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const token = this.getToken()
    return `${protocol}//${window.location.host}/ws/terminal/${workerId}?token=${token}`
  }

  getScreenWsUrl(workerId: string): string {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const token = this.getToken()
    return `${protocol}//${window.location.host}/ws/screen/${workerId}?token=${token}`
  }

  // OAuth Client Management
  async listOAuthClients(): Promise<OAuthClient[]> {
    const data = await this.fetch<{ clients: OAuthClient[] }>('/oauth/clients')
    return data.clients
  }

  async createOAuthClient(name: string, scopes: string[] = ['read', 'write']): Promise<OAuthClient> {
    return this.fetch('/oauth/clients', {
      method: 'POST',
      body: JSON.stringify({ name, scopes }),
    })
  }

  async revokeOAuthClient(clientId: string): Promise<void> {
    await this.fetch(`/oauth/clients/${clientId}`, {
      method: 'DELETE',
    })
  }

  getMcpUrl(): string {
    return `${getServerBaseUrl()}/api/mcp/sse`
  }

  // Account Management
  async changePassword(currentPassword: string, newPassword: string): Promise<void> {
    await this.fetch('/auth/change-password', {
      method: 'POST',
      body: JSON.stringify({ current_password: currentPassword, new_password: newPassword }),
    })
  }

  async changeUsername(newUsername: string): Promise<{ new_username: string; access_token: string }> {
    return this.fetch('/auth/change-username', {
      method: 'POST',
      body: JSON.stringify({ new_username: newUsername }),
    })
  }

  // Collection API
  async getCollectionConfig(workerId: string): Promise<CollectionConfig> {
    return this.fetch(`/workers/${workerId}/collection/config`)
  }

  async setCollectionConfig(workerId: string, config: CollectionConfig): Promise<CollectionConfig> {
    return this.fetch(`/workers/${workerId}/collection/config`, {
      method: 'PUT',
      body: JSON.stringify(config),
    })
  }

  async startCollection(workerId: string): Promise<{ status: string; session_id?: string }> {
    return this.fetch(`/workers/${workerId}/collection/start`, {
      method: 'POST',
    })
  }

  async stopCollection(workerId: string): Promise<{ status: string }> {
    return this.fetch(`/workers/${workerId}/collection/stop`, {
      method: 'POST',
    })
  }

  async getCollectionStatus(workerId: string): Promise<CollectionStatus> {
    return this.fetch(`/workers/${workerId}/collection/status`)
  }

  async listCollectionSessions(workerId: string, limit = 50): Promise<CollectionSession[]> {
    const data = await this.fetch<{ sessions: CollectionSession[] }>(
      `/workers/${workerId}/collection/sessions?limit=${limit}`
    )
    return data.sessions
  }

  async listSessionChunks(workerId: string, sessionId: string): Promise<CollectionChunk[]> {
    const data = await this.fetch<{ chunks: CollectionChunk[] }>(
      `/workers/${workerId}/collection/sessions/${sessionId}/chunks`
    )
    return data.chunks
  }

  getChunkVideoUrl(workerId: string, chunkId: string, stream = 'display_0/video.raw'): string {
    const token = this.getToken()
    return `${API_BASE}/workers/${workerId}/collection/chunks/${chunkId}/video?stream=${encodeURIComponent(stream)}&token=${token}`
  }

  async setEncryptionKey(workerId: string, password: string): Promise<{ status: string; message: string }> {
    return this.fetch(`/workers/${workerId}/collection/encryption-key`, {
      method: 'POST',
      body: JSON.stringify({ password }),
    })
  }

  // Admin API for cache and R2
  async getCacheStats(): Promise<CacheStats> {
    return this.fetch('/admin/cache/stats')
  }

  async listCacheChunks(workerId?: string): Promise<CollectionChunk[]> {
    const params = workerId ? `?worker_id=${workerId}` : ''
    const data = await this.fetch<{ chunks: CollectionChunk[] }>(`/admin/cache/chunks${params}`)
    return data.chunks
  }

  async setR2Config(
    endpoint: string,
    accessKey: string,
    secretKey: string,
    bucketNormal: string,
    bucketInfrequent?: string
  ): Promise<{ status: string }> {
    return this.fetch('/admin/r2-config', {
      method: 'PUT',
      body: JSON.stringify({
        endpoint,
        access_key: accessKey,
        secret_key: secretKey,
        bucket_normal: bucketNormal,
        bucket_infrequent: bucketInfrequent,
      }),
    })
  }
}

export const api = new CorelyAPI()
export type {
  Worker,
  SystemInfo,
  ScreenCapture,
  ShellResult,
  OAuthClient,
  CollectionConfig,
  ScreenConfig,
  CameraConfig,
  AudioInputConfig,
  AudioOutputConfig,
  InputLoggingConfig,
  DirectorySyncConfig,
  CollectionStatus,
  CollectionSession,
  CollectionChunk,
  CacheStats,
}
