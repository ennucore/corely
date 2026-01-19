import { useState, useEffect, useRef, useCallback } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { colors } from '../styles'
import { api, Worker, SystemInfo } from '../api/client'
import { CRTFrame } from '../components/CRTFrame'
import { GlowButton } from '../components/GlowButton'
import { DataRow, ProgressBar } from '../components/DataDisplay'

type Tab = 'overview' | 'screen' | 'terminal' | 'files'

export function WorkerDetailPage() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const [worker, setWorker] = useState<Worker | null>(null)
  const [systemInfo, setSystemInfo] = useState<SystemInfo | null>(null)
  const [activeTab, setActiveTab] = useState<Tab>('overview')
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState('')

  // Fetch worker data
  useEffect(() => {
    if (!id) return

    const fetchData = async () => {
      try {
        const workerData = await api.getWorker(id)
        setWorker(workerData)

        if (workerData.is_online) {
          const sysInfo = await api.getSystemInfo(id)
          setSystemInfo(sysInfo)
        }
        setError('')
      } catch (err) {
        setError('Failed to fetch worker data')
      } finally {
        setIsLoading(false)
      }
    }

    fetchData()
    const interval = setInterval(fetchData, 10000)
    return () => clearInterval(interval)
  }, [id])

  const formatBytes = (bytes: number) => {
    const units = ['B', 'KB', 'MB', 'GB', 'TB']
    let unitIndex = 0
    let size = bytes
    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024
      unitIndex++
    }
    return `${size.toFixed(1)} ${units[unitIndex]}`
  }

  const formatUptime = (seconds: number) => {
    const days = Math.floor(seconds / 86400)
    const hours = Math.floor((seconds % 86400) / 3600)
    const minutes = Math.floor((seconds % 3600) / 60)
    return `${days}d ${hours}h ${minutes}m`
  }

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
          ◐ LOADING WORKER DATA...
        </div>
      </div>
    )
  }

  if (!worker) {
    return (
      <div style={{
        minHeight: '100vh',
        background: colors.void,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        flexDirection: 'column',
        gap: 20,
      }}>
        <div style={{
          fontFamily: 'VT323, monospace',
          fontSize: '24px',
          color: colors.red,
        }}>
          ⚠ WORKER NOT FOUND
        </div>
        <GlowButton onClick={() => navigate('/')}>
          ← RETURN TO DASHBOARD
        </GlowButton>
      </div>
    )
  }

  const tabs: { id: Tab; label: string; icon: string }[] = [
    { id: 'overview', label: 'SYSTEM', icon: '◉' },
    { id: 'screen', label: 'SCREEN', icon: '▣' },
    { id: 'terminal', label: 'TERMINAL', icon: '▤' },
    { id: 'files', label: 'FILES', icon: '▥' },
  ]

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
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
          <GlowButton size="small" onClick={() => navigate('/')}>
            ← BACK
          </GlowButton>
          <GlowButton size="small" variant="magenta" onClick={() => navigate(`/worker/${id}/collection`)}>
            DATA COLLECTION
          </GlowButton>
          <div>
            <div style={{
              fontFamily: 'VT323, monospace',
              fontSize: '24px',
              color: colors.cyan,
              textShadow: colors.cyanGlow,
            }}>
              {worker.name}
            </div>
            <div style={{
              fontSize: '10px',
              color: colors.textMuted,
            }}>
              {worker.id}
            </div>
          </div>
        </div>

        <div style={{
          display: 'flex',
          alignItems: 'center',
          gap: 8,
        }}>
          <div style={{
            width: 10,
            height: 10,
            borderRadius: '50%',
            background: worker.is_online ? colors.green : colors.red,
            boxShadow: worker.is_online ? colors.greenGlow : colors.redGlow,
            animation: worker.is_online ? 'pulse 2s infinite' : 'none',
          }} />
          <span style={{
            fontSize: '12px',
            color: worker.is_online ? colors.green : colors.red,
            textTransform: 'uppercase',
          }}>
            {worker.is_online ? 'ONLINE' : 'OFFLINE'}
          </span>
        </div>
      </header>

      {/* Tab navigation */}
      <nav style={{
        background: colors.surface,
        borderBottom: `1px solid ${colors.panel}`,
        display: 'flex',
        padding: '0 24px',
      }}>
        {tabs.map(tab => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            disabled={!worker.is_online && tab.id !== 'overview'}
            style={{
              padding: '16px 24px',
              background: activeTab === tab.id ? colors.deep : 'transparent',
              border: 'none',
              borderBottom: activeTab === tab.id ? `2px solid ${colors.cyan}` : '2px solid transparent',
              color: activeTab === tab.id ? colors.cyan : colors.textMuted,
              fontFamily: 'Share Tech Mono, monospace',
              fontSize: '12px',
              letterSpacing: '2px',
              cursor: worker.is_online || tab.id === 'overview' ? 'pointer' : 'not-allowed',
              opacity: !worker.is_online && tab.id !== 'overview' ? 0.5 : 1,
              transition: 'all 0.2s ease',
            }}
          >
            <span style={{ marginRight: 8 }}>{tab.icon}</span>
            {tab.label}
          </button>
        ))}
      </nav>

      {/* Content */}
      <main style={{
        flex: 1,
        padding: 24,
        overflow: 'auto',
      }}>
        {error && (
          <div style={{
            background: `${colors.red}22`,
            border: `1px solid ${colors.red}`,
            padding: '16px 20px',
            marginBottom: 24,
            color: colors.red,
          }}>
            ⚠ {error}
          </div>
        )}

        {activeTab === 'overview' && (
          <OverviewTab worker={worker} systemInfo={systemInfo} formatBytes={formatBytes} formatUptime={formatUptime} />
        )}
        {activeTab === 'screen' && worker.is_online && (
          <ScreenTab workerId={worker.id} />
        )}
        {activeTab === 'terminal' && worker.is_online && (
          <TerminalTab workerId={worker.id} />
        )}
        {activeTab === 'files' && worker.is_online && (
          <FilesTab workerId={worker.id} />
        )}
      </main>
    </div>
  )
}

// Overview Tab
function OverviewTab({ worker, systemInfo, formatBytes, formatUptime }: {
  worker: Worker
  systemInfo: SystemInfo | null
  formatBytes: (b: number) => string
  formatUptime: (s: number) => string
}) {
  return (
    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 24 }}>
      {/* Worker Info */}
      <CRTFrame title="WORKER INFO">
        <DataRow label="Name" value={worker.name} />
        <DataRow label="Hostname" value={worker.hostname || 'N/A'} />
        <DataRow label="OS" value={worker.os || 'N/A'} />
        <DataRow label="Architecture" value={worker.arch || 'N/A'} />
        <DataRow label="Capabilities" value={worker.capabilities?.length || 0} />
        {worker.last_seen && (
          <DataRow label="Last Seen" value={new Date(worker.last_seen).toLocaleString()} />
        )}
      </CRTFrame>

      {/* System Stats */}
      {systemInfo && (
        <>
          <CRTFrame title="SYSTEM STATUS">
            <DataRow label="Hostname" value={systemInfo.hostname} />
            <DataRow label="OS" value={`${systemInfo.os.name} ${systemInfo.os.version}`} />
            <DataRow label="Kernel" value={systemInfo.os.kernel_version} />
            <DataRow label="Uptime" value={formatUptime(systemInfo.uptime)} color={colors.green} />
          </CRTFrame>

          <CRTFrame title="CPU">
            <DataRow label="Model" value={systemInfo.cpu.brand || 'Unknown'} />
            <DataRow label="Cores" value={systemInfo.cpu.cores} />
            <ProgressBar
              label="Usage"
              value={systemInfo.cpu.usage_percent}
              color={systemInfo.cpu.usage_percent > 80 ? colors.red : colors.cyan}
            />
          </CRTFrame>

          <CRTFrame title="MEMORY">
            <DataRow label="Total" value={formatBytes(systemInfo.memory.total)} />
            <DataRow label="Used" value={formatBytes(systemInfo.memory.used)} color={colors.amber} />
            <DataRow label="Available" value={formatBytes(systemInfo.memory.available)} color={colors.green} />
            <ProgressBar
              label="Usage"
              value={systemInfo.memory.usage_percent}
              color={systemInfo.memory.usage_percent > 80 ? colors.red : colors.magenta}
            />
          </CRTFrame>

          {systemInfo.disks.length > 0 && (
            <CRTFrame title="STORAGE" style={{ gridColumn: 'span 2' }}>
              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(250px, 1fr))', gap: 16 }}>
                {systemInfo.disks.map((disk, i) => (
                  <div key={i} style={{
                    background: colors.panel,
                    padding: 16,
                    border: `1px solid ${colors.textMuted}`,
                  }}>
                    <div style={{
                      fontSize: '12px',
                      color: colors.cyan,
                      marginBottom: 8,
                    }}>
                      {disk.mount_point}
                    </div>
                    <ProgressBar
                      value={((disk.total_space - disk.available_space) / disk.total_space) * 100}
                      color={colors.amber}
                      showValue={false}
                    />
                    <div style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      fontSize: '10px',
                      color: colors.textMuted,
                      marginTop: 4,
                    }}>
                      <span>{formatBytes(disk.total_space - disk.available_space)} used</span>
                      <span>{formatBytes(disk.available_space)} free</span>
                    </div>
                  </div>
                ))}
              </div>
            </CRTFrame>
          )}
        </>
      )}
    </div>
  )
}

// Screen Tab
function ScreenTab({ workerId }: { workerId: string }) {
  const [screenshot, setScreenshot] = useState<string | null>(null)
  const [isLoading, setIsLoading] = useState(false)
  const [autoRefresh, setAutoRefresh] = useState(false)
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const captureScreen = useCallback(async () => {
    setIsLoading(true)
    try {
      const result = await api.getScreen(workerId)
      setScreenshot(`data:image/png;base64,${result.data}`)
    } catch (err) {
      console.error('Failed to capture screen:', err)
    } finally {
      setIsLoading(false)
    }
  }, [workerId])

  useEffect(() => {
    captureScreen()
  }, [captureScreen])

  useEffect(() => {
    if (autoRefresh) {
      intervalRef.current = setInterval(captureScreen, 2000)
    } else if (intervalRef.current) {
      clearInterval(intervalRef.current)
    }
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current)
    }
  }, [autoRefresh, captureScreen])

  return (
    <CRTFrame title="SCREEN CAPTURE" subtitle="Live view of remote display">
      <div style={{
        display: 'flex',
        gap: 12,
        marginBottom: 16,
      }}>
        <GlowButton onClick={captureScreen} disabled={isLoading}>
          {isLoading ? '◐ CAPTURING...' : '⟳ REFRESH'}
        </GlowButton>
        <GlowButton
          onClick={() => setAutoRefresh(!autoRefresh)}
          variant={autoRefresh ? 'green' : 'cyan'}
        >
          {autoRefresh ? '◉ AUTO-REFRESH ON' : '○ AUTO-REFRESH OFF'}
        </GlowButton>
      </div>

      <div style={{
        background: colors.void,
        border: `1px solid ${colors.panel}`,
        padding: 4,
        position: 'relative',
        minHeight: 400,
      }}>
        {screenshot ? (
          <img
            src={screenshot}
            alt="Remote screen"
            style={{
              width: '100%',
              height: 'auto',
              display: 'block',
              imageRendering: 'auto',
            }}
          />
        ) : (
          <div style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            height: 400,
            color: colors.textMuted,
          }}>
            {isLoading ? '◐ LOADING...' : 'No screenshot available'}
          </div>
        )}

        {/* Scanline overlay */}
        <div style={{
          position: 'absolute',
          top: 0,
          left: 0,
          right: 0,
          bottom: 0,
          background: 'repeating-linear-gradient(0deg, transparent, transparent 2px, rgba(0,0,0,0.1) 2px, rgba(0,0,0,0.1) 4px)',
          pointerEvents: 'none',
        }} />
      </div>
    </CRTFrame>
  )
}

// Terminal Tab
function TerminalTab({ workerId }: { workerId: string }) {
  const [command, setCommand] = useState('')
  const [output, setOutput] = useState<Array<{ type: 'input' | 'output' | 'error'; text: string }>>([])
  const [isRunning, setIsRunning] = useState(false)
  const outputRef = useRef<HTMLDivElement>(null)

  const runCommand = async () => {
    if (!command.trim() || isRunning) return

    setOutput(prev => [...prev, { type: 'input', text: `$ ${command}` }])
    setIsRunning(true)

    try {
      const result = await api.execShell(workerId, command)
      if (result.stdout) {
        setOutput(prev => [...prev, { type: 'output', text: result.stdout }])
      }
      if (result.stderr) {
        setOutput(prev => [...prev, { type: 'error', text: result.stderr }])
      }
      if (result.exit_code !== 0) {
        setOutput(prev => [...prev, { type: 'error', text: `Exit code: ${result.exit_code}` }])
      }
    } catch (err) {
      setOutput(prev => [...prev, { type: 'error', text: `Error: ${err}` }])
    } finally {
      setIsRunning(false)
      setCommand('')
    }
  }

  useEffect(() => {
    if (outputRef.current) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight
    }
  }, [output])

  return (
    <CRTFrame title="TERMINAL" subtitle="Remote shell access">
      <div
        ref={outputRef}
        style={{
          background: colors.void,
          border: `1px solid ${colors.panel}`,
          padding: 16,
          height: 400,
          overflow: 'auto',
          fontFamily: 'Share Tech Mono, monospace',
          fontSize: '13px',
          marginBottom: 16,
        }}
      >
        {output.map((line, i) => (
          <div
            key={i}
            style={{
              color: line.type === 'input' ? colors.cyan :
                     line.type === 'error' ? colors.red : colors.green,
              whiteSpace: 'pre-wrap',
              marginBottom: 4,
            }}
          >
            {line.text}
          </div>
        ))}
        {isRunning && (
          <div style={{ color: colors.amber, animation: 'pulse 1s infinite' }}>
            ◐ Running...
          </div>
        )}
      </div>

      <div style={{ display: 'flex', gap: 12 }}>
        <input
          type="text"
          value={command}
          onChange={(e) => setCommand(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && runCommand()}
          placeholder="Enter command..."
          style={{
            flex: 1,
            padding: '12px 16px',
            background: colors.surface,
            border: `1px solid ${colors.textMuted}`,
            color: colors.cyan,
            fontFamily: 'Share Tech Mono, monospace',
            fontSize: '14px',
            outline: 'none',
          }}
        />
        <GlowButton onClick={runCommand} disabled={isRunning || !command.trim()}>
          ► EXECUTE
        </GlowButton>
      </div>
    </CRTFrame>
  )
}

// Files Tab (placeholder)
function FilesTab({ workerId }: { workerId: string }) {
  const [path, setPath] = useState('/')
  const [files, setFiles] = useState<string[]>([])

  useEffect(() => {
    const fetchFiles = async () => {
      try {
        const result = await api.callWorker(workerId, 'fs.glob', { pattern: '*', path }) as { matches: string[] }
        setFiles(result.matches || [])
      } catch (err) {
        console.error('Failed to list files:', err)
      }
    }
    fetchFiles()
  }, [workerId, path])

  return (
    <CRTFrame title="FILE BROWSER" subtitle={`Current: ${path}`}>
      <div style={{
        display: 'flex',
        gap: 12,
        marginBottom: 16,
      }}>
        <input
          type="text"
          value={path}
          onChange={(e) => setPath(e.target.value)}
          style={{
            flex: 1,
            padding: '10px 16px',
            background: colors.surface,
            border: `1px solid ${colors.textMuted}`,
            color: colors.cyan,
            fontFamily: 'Share Tech Mono, monospace',
            fontSize: '14px',
            outline: 'none',
          }}
        />
        <GlowButton onClick={() => setPath(path)}>
          ► GO
        </GlowButton>
      </div>

      <div style={{
        background: colors.void,
        border: `1px solid ${colors.panel}`,
        padding: 16,
        maxHeight: 400,
        overflow: 'auto',
      }}>
        {files.length > 0 ? (
          files.map((file, i) => (
            <div
              key={i}
              style={{
                padding: '8px 12px',
                borderBottom: `1px solid ${colors.panel}`,
                color: colors.textPrimary,
                fontSize: '13px',
                cursor: 'pointer',
              }}
              onClick={() => {
                if (!file.includes('.')) {
                  setPath(file)
                }
              }}
            >
              {file.includes('.') ? '📄' : '📁'} {file}
            </div>
          ))
        ) : (
          <div style={{ color: colors.textMuted, textAlign: 'center', padding: 40 }}>
            No files found
          </div>
        )}
      </div>
    </CRTFrame>
  )
}
