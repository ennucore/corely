import { useState, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { colors } from '../styles'
import { api, Worker, getServerBaseUrl } from '../api/client'
import { CRTFrame } from '../components/CRTFrame'
import { WorkerCard } from '../components/WorkerCard'
import { GlowButton } from '../components/GlowButton'
import { useAuth } from '../hooks/useAuth'

export function DashboardPage() {
  const navigate = useNavigate()
  const { user, logout } = useAuth()
  const [workers, setWorkers] = useState<Worker[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState('')
  const [currentTime, setCurrentTime] = useState(new Date())
  const [showInstallCommand, setShowInstallCommand] = useState(false)
  const [copied, setCopied] = useState(false)

  // Generate install command based on API server URL
  const serverUrl = getServerBaseUrl()
  const installCommand = `curl -fsSL ${serverUrl}/install.sh | bash`

  const copyToClipboard = () => {
    navigator.clipboard.writeText(installCommand)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  // Update time every second
  useEffect(() => {
    const timer = setInterval(() => setCurrentTime(new Date()), 1000)
    return () => clearInterval(timer)
  }, [])

  // Fetch workers
  useEffect(() => {
    const fetchWorkers = async () => {
      try {
        const data = await api.listWorkers()
        setWorkers(data)
        setError('')
      } catch (err) {
        setError('Failed to fetch workers')
      } finally {
        setIsLoading(false)
      }
    }

    fetchWorkers()
    const interval = setInterval(fetchWorkers, 5000)
    return () => clearInterval(interval)
  }, [])

  const onlineWorkers = workers.filter(w => w.is_online)
  const offlineWorkers = workers.filter(w => !w.is_online)

  return (
    <div style={{
      minHeight: '100vh',
      background: colors.void,
      display: 'flex',
      flexDirection: 'column',
    }}>
      {/* Header */}
      <header style={{
        background: colors.deep,
        borderBottom: `1px solid ${colors.cyan}`,
        padding: '12px 24px',
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        position: 'sticky',
        top: 0,
        zIndex: 100,
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 24 }}>
          <div style={{
            fontFamily: 'VT323, monospace',
            fontSize: '28px',
            color: colors.cyan,
            textShadow: colors.cyanGlow,
            letterSpacing: '4px',
          }}>
            CORELY
          </div>
          <div style={{
            fontSize: '11px',
            color: colors.textMuted,
            borderLeft: `1px solid ${colors.panel}`,
            paddingLeft: 24,
          }}>
            REMOTE SYSTEMS CONTROL
          </div>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 24 }}>
          {/* Clock */}
          <div style={{
            fontFamily: 'VT323, monospace',
            fontSize: '18px',
            color: colors.green,
            textShadow: colors.greenGlow,
          }}>
            {currentTime.toLocaleTimeString('en-US', { hour12: false })}
          </div>

          {/* User info */}
          <div style={{
            fontSize: '11px',
            color: colors.textSecondary,
          }}>
            <span style={{ color: colors.textMuted }}>USER:</span> {user?.username.toUpperCase()}
          </div>

          <GlowButton size="small" variant="cyan" onClick={() => navigate('/settings')}>
            MCP
          </GlowButton>

          <GlowButton size="small" variant="red" onClick={logout}>
            LOGOUT
          </GlowButton>
        </div>
      </header>

      {/* Main content */}
      <main style={{
        flex: 1,
        padding: 24,
        overflowY: 'auto',
      }}>
        {/* Compact stats bar with install command */}
        <div style={{
          display: 'flex',
          alignItems: 'center',
          gap: 24,
          padding: '12px 16px',
          background: colors.panel,
          border: `1px solid ${colors.textMuted}`,
          marginBottom: 24,
        }}>
          {/* Stats */}
          <div style={{ display: 'flex', alignItems: 'center', gap: 20 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <span style={{ color: colors.cyan, fontFamily: 'VT323, monospace', fontSize: '20px', textShadow: `0 0 5px ${colors.cyan}` }}>
                {workers.length}
              </span>
              <span style={{ color: colors.textMuted, fontSize: '10px', textTransform: 'uppercase', letterSpacing: '1px' }}>
                Total
              </span>
            </div>
            <div style={{ width: 1, height: 20, background: colors.textMuted }} />
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <span style={{ color: colors.green, fontFamily: 'VT323, monospace', fontSize: '20px', textShadow: `0 0 5px ${colors.green}` }}>
                {onlineWorkers.length}
              </span>
              <span style={{ color: colors.textMuted, fontSize: '10px', textTransform: 'uppercase', letterSpacing: '1px' }}>
                Online
              </span>
            </div>
            <div style={{ width: 1, height: 20, background: colors.textMuted }} />
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <span style={{ color: colors.red, fontFamily: 'VT323, monospace', fontSize: '20px', textShadow: `0 0 5px ${colors.red}` }}>
                {offlineWorkers.length}
              </span>
              <span style={{ color: colors.textMuted, fontSize: '10px', textTransform: 'uppercase', letterSpacing: '1px' }}>
                Offline
              </span>
            </div>
          </div>

          {/* Spacer */}
          <div style={{ flex: 1 }} />

          {/* Install command */}
          {showInstallCommand ? (
            <div style={{ display: 'flex', alignItems: 'center', gap: 12, animation: 'bootSequence 0.2s ease' }}>
              <code style={{
                fontFamily: 'Share Tech Mono, monospace',
                fontSize: '11px',
                color: colors.green,
                background: colors.void,
                padding: '6px 10px',
                border: `1px solid ${colors.cyan}`,
                maxWidth: 400,
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
              }}>
                {installCommand}
              </code>
              <GlowButton size="small" variant="green" onClick={copyToClipboard}>
                {copied ? '✓' : '📋'}
              </GlowButton>
              <GlowButton size="small" variant="cyan" onClick={() => setShowInstallCommand(false)}>
                ✕
              </GlowButton>
            </div>
          ) : (
            <GlowButton size="small" variant="cyan" onClick={() => setShowInstallCommand(true)}>
              + ADD WORKER
            </GlowButton>
          )}
        </div>

        {/* Error display */}
        {error && (
          <div style={{
            background: `${colors.red}22`,
            border: `1px solid ${colors.red}`,
            padding: '16px 20px',
            marginBottom: 24,
            color: colors.red,
            fontSize: '13px',
          }}>
            ⚠ {error}
          </div>
        )}

        {/* Loading state */}
        {isLoading && (
          <div style={{
            textAlign: 'center',
            padding: 60,
            color: colors.cyan,
            fontFamily: 'VT323, monospace',
            fontSize: '18px',
            animation: 'pulse 1s infinite',
          }}>
            ◐ SCANNING NETWORK FOR WORKERS...
          </div>
        )}

        {/* Workers section */}
        {!isLoading && (
          <>
            {/* Online workers */}
            {onlineWorkers.length > 0 && (
              <CRTFrame
                title="ACTIVE NODES"
                subtitle={`${onlineWorkers.length} worker(s) connected`}
                status="online"
                style={{ marginBottom: 24 }}
              >
                <div style={{
                  display: 'grid',
                  gridTemplateColumns: 'repeat(auto-fill, minmax(300px, 1fr))',
                  gap: 16,
                }}>
                  {onlineWorkers.map(worker => (
                    <WorkerCard
                      key={worker.id}
                      worker={worker}
                      onClick={() => navigate(`/worker/${worker.id}`)}
                    />
                  ))}
                </div>
              </CRTFrame>
            )}

            {/* Offline workers */}
            {offlineWorkers.length > 0 && (
              <CRTFrame
                title="INACTIVE NODES"
                subtitle={`${offlineWorkers.length} worker(s) offline`}
                status="offline"
              >
                <div style={{
                  display: 'grid',
                  gridTemplateColumns: 'repeat(auto-fill, minmax(300px, 1fr))',
                  gap: 16,
                }}>
                  {offlineWorkers.map(worker => (
                    <WorkerCard
                      key={worker.id}
                      worker={worker}
                      onClick={() => navigate(`/worker/${worker.id}`)}
                    />
                  ))}
                </div>
              </CRTFrame>
            )}

            {/* Empty state */}
            {workers.length === 0 && (
              <div style={{
                textAlign: 'center',
                padding: 80,
                border: `1px dashed ${colors.textMuted}`,
              }}>
                <div style={{
                  fontSize: '48px',
                  marginBottom: 16,
                  opacity: 0.5,
                }}>
                  📡
                </div>
                <div style={{
                  fontFamily: 'VT323, monospace',
                  fontSize: '24px',
                  color: colors.textMuted,
                  marginBottom: 8,
                }}>
                  NO WORKERS DETECTED
                </div>
                <div style={{
                  fontSize: '12px',
                  color: colors.textMuted,
                }}>
                  Deploy a worker to get started
                </div>
              </div>
            )}
          </>
        )}
      </main>

      {/* Footer */}
      <footer style={{
        background: colors.deep,
        borderTop: `1px solid ${colors.panel}`,
        padding: '8px 24px',
        fontSize: '10px',
        color: colors.textMuted,
        display: 'flex',
        justifyContent: 'space-between',
      }}>
        <span>CORELY SYSTEMS v0.1.0</span>
        <span>◆ SECURE CONNECTION ◆ ENCRYPTED CHANNEL ◆</span>
        <span>{currentTime.toLocaleDateString()}</span>
      </footer>
    </div>
  )
}
