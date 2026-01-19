import { useState, useEffect } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { colors } from '../styles'
import { api, CollectionConfig, CollectionStatus, Worker } from '../api/client'
import { CRTFrame } from '../components/CRTFrame'
import { GlowButton } from '../components/GlowButton'
import { ConfigEditor } from '../components/collection/ConfigEditor'
import { SessionBrowser } from '../components/collection/SessionBrowser'
import { EncryptionSetup } from '../components/collection/EncryptionSetup'

type Tab = 'config' | 'sessions' | 'encryption'

const defaultConfig: CollectionConfig = {
  screen: {
    enabled: false,
    fps: 1,
    resolution: 720,
    all_displays: true,
    display_ids: [],
    quality: 80,
  },
  camera: {
    enabled: false,
    fps: 5,
    resolution: 480,
    all_cameras: true,
    camera_indices: [],
    implies_mic: true,
  },
  audio_input: {
    enabled: false,
    sample_rate: 44100,
    device: null,
  },
  audio_output: {
    enabled: false,
    sample_rate: 44100,
    device: null,
  },
  input_logging: {
    enabled: false,
    log_keystrokes: true,
    log_mouse_moves: true,
    log_mouse_clicks: true,
    mouse_sample_ms: 100,
  },
  directory_sync: {
    paths: [],
    include_patterns: [],
    exclude_patterns: [],
    sync_interval_secs: 300,
    max_file_size: 100 * 1024 * 1024,
    watch_changes: true,
  },
  chunk_duration_secs: 60,
  output_dir: '/tmp/corely_collection',
}

export function CollectionConfigPage() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const [worker, setWorker] = useState<Worker | null>(null)
  const [config, setConfig] = useState<CollectionConfig>(defaultConfig)
  const [status, setStatus] = useState<CollectionStatus | null>(null)
  const [activeTab, setActiveTab] = useState<Tab>('config')
  const [isLoading, setIsLoading] = useState(true)
  const [isSaving, setIsSaving] = useState(false)
  const [error, setError] = useState('')
  const [success, setSuccess] = useState('')

  useEffect(() => {
    if (!id) return
    loadData()
    const interval = setInterval(loadStatus, 5000)
    return () => clearInterval(interval)
  }, [id])

  const loadData = async () => {
    if (!id) return
    try {
      const [workerData, configData, statusData] = await Promise.all([
        api.getWorker(id),
        api.getCollectionConfig(id),
        api.getCollectionStatus(id),
      ])
      setWorker(workerData)
      setConfig(configData)
      setStatus(statusData)
    } catch (err) {
      setError('Failed to load data')
    } finally {
      setIsLoading(false)
    }
  }

  const loadStatus = async () => {
    if (!id) return
    try {
      const statusData = await api.getCollectionStatus(id)
      setStatus(statusData)
    } catch (err) {
      // Ignore status fetch errors
    }
  }

  const handleSave = async () => {
    if (!id) return
    setIsSaving(true)
    setError('')
    setSuccess('')
    try {
      await api.setCollectionConfig(id, config)
      setSuccess('Configuration saved')
      setTimeout(() => setSuccess(''), 3000)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save configuration')
    } finally {
      setIsSaving(false)
    }
  }

  const handleStart = async () => {
    if (!id) return
    try {
      await api.startCollection(id)
      loadStatus()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to start collection')
    }
  }

  const handleStop = async () => {
    if (!id) return
    try {
      await api.stopCollection(id)
      loadStatus()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to stop collection')
    }
  }

  const tabStyle = (tab: Tab) => ({
    fontFamily: 'VT323, monospace',
    fontSize: '18px',
    padding: '12px 24px',
    background: activeTab === tab ? colors.surface : 'transparent',
    color: activeTab === tab ? colors.cyan : colors.textPrimary,
    border: `1px solid ${activeTab === tab ? colors.cyan : colors.cyan}40`,
    borderBottom: activeTab === tab ? 'none' : `1px solid ${colors.cyan}40`,
    borderRadius: '8px 8px 0 0',
    cursor: 'pointer',
    marginRight: '5px',
    transition: 'all 0.2s',
    textShadow: activeTab === tab ? `0 0 10px ${colors.cyan}` : 'none',
  })

  if (isLoading) {
    return (
      <div style={{
        minHeight: '100vh',
        background: colors.void,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      }}>
        <div style={{
          fontFamily: 'VT323, monospace',
          fontSize: '24px',
          color: colors.cyan,
          animation: 'pulse 1s infinite',
        }}>
          LOADING COLLECTION CONFIG...
        </div>
      </div>
    )
  }

  return (
    <div style={{
      minHeight: '100vh',
      background: colors.void,
      padding: '20px',
    }}>
      <div style={{ maxWidth: '1200px', margin: '0 auto' }}>
        {/* Header */}
        <div style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          marginBottom: '30px',
        }}>
          <div>
            <GlowButton onClick={() => navigate(`/worker/${id}`)} variant="cyan" style={{ marginRight: '15px' }}>
              BACK TO WORKER
            </GlowButton>
            <span style={{
              fontFamily: 'VT323, monospace',
              fontSize: '28px',
              color: colors.cyan,
              textShadow: `0 0 20px ${colors.cyan}`,
            }}>
              DATA COLLECTION: {worker?.name || id}
            </span>
          </div>
        </div>

        {/* Status Bar */}
        <CRTFrame>
          <div style={{ padding: '20px' }}>
            <div style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
            }}>
              <div>
                <span style={{
                  fontFamily: 'VT323, monospace',
                  fontSize: '20px',
                  color: colors.textPrimary,
                }}>
                  STATUS:{' '}
                  <span style={{
                    color: status?.is_collecting ? colors.green : colors.textPrimary,
                    textShadow: status?.is_collecting ? `0 0 10px ${colors.green}` : 'none',
                  }}>
                    {status?.is_collecting ? 'COLLECTING' : 'IDLE'}
                  </span>
                </span>
                {status?.session_id && (
                  <span style={{
                    fontFamily: 'VT323, monospace',
                    fontSize: '16px',
                    color: colors.textPrimary,
                    marginLeft: '20px',
                  }}>
                    Session: {status.session_id.substring(0, 20)}...
                  </span>
                )}
                {status?.chunk_count !== undefined && status.chunk_count > 0 && (
                  <span style={{
                    fontFamily: 'VT323, monospace',
                    fontSize: '16px',
                    color: colors.cyan,
                    marginLeft: '20px',
                  }}>
                    Chunks: {status.chunk_count}
                  </span>
                )}
              </div>
              <div style={{ display: 'flex', gap: '15px' }}>
                {!status?.is_collecting ? (
                  <GlowButton onClick={handleStart} variant="green" disabled={!worker?.is_online}>
                    START COLLECTION
                  </GlowButton>
                ) : (
                  <GlowButton onClick={handleStop} variant="red">
                    STOP COLLECTION
                  </GlowButton>
                )}
              </div>
            </div>
            {!worker?.is_online && (
              <div style={{
                fontFamily: 'VT323, monospace',
                fontSize: '14px',
                color: colors.amber,
                marginTop: '10px',
              }}>
                Worker is offline. Collection controls will be available when the worker reconnects.
              </div>
            )}
          </div>
        </CRTFrame>

        {/* Alerts */}
        {error && (
          <div style={{
            fontFamily: 'VT323, monospace',
            fontSize: '16px',
            color: colors.red,
            background: `${colors.red}20`,
            border: `1px solid ${colors.red}50`,
            borderRadius: '4px',
            padding: '10px 15px',
            marginTop: '15px',
          }}>
            {error}
          </div>
        )}
        {success && (
          <div style={{
            fontFamily: 'VT323, monospace',
            fontSize: '16px',
            color: colors.green,
            background: `${colors.green}20`,
            border: `1px solid ${colors.green}50`,
            borderRadius: '4px',
            padding: '10px 15px',
            marginTop: '15px',
          }}>
            {success}
          </div>
        )}

        {/* Tabs */}
        <div style={{ marginTop: '30px' }}>
          <div style={{ display: 'flex', borderBottom: `1px solid ${colors.cyan}40` }}>
            <button style={tabStyle('config')} onClick={() => setActiveTab('config')}>
              CONFIGURATION
            </button>
            <button style={tabStyle('sessions')} onClick={() => setActiveTab('sessions')}>
              SESSIONS
            </button>
            <button style={tabStyle('encryption')} onClick={() => setActiveTab('encryption')}>
              ENCRYPTION
            </button>
          </div>

          {/* Tab Content */}
          <CRTFrame>
            <div style={{ padding: '25px' }}>
              {activeTab === 'config' && (
                <ConfigEditor
                  config={config}
                  onChange={setConfig}
                  onSave={handleSave}
                  isSaving={isSaving}
                />
              )}
              {activeTab === 'sessions' && id && (
                <SessionBrowser workerId={id} />
              )}
              {activeTab === 'encryption' && id && (
                <EncryptionSetup workerId={id} />
              )}
            </div>
          </CRTFrame>
        </div>
      </div>
    </div>
  )
}
