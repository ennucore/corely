import { useState, useEffect } from 'react'
import { colors } from '../../styles'
import { api, CollectionSession, CollectionChunk } from '../../api/client'

interface SessionBrowserProps {
  workerId: string
  onSelectChunk?: (chunk: CollectionChunk) => void
}

const sessionItemStyle = {
  background: `${colors.surface}60`,
  border: `1px solid ${colors.cyan}30`,
  borderRadius: '6px',
  padding: '15px',
  marginBottom: '10px',
  cursor: 'pointer',
  transition: 'all 0.2s',
}

const sessionItemHoverStyle = {
  ...sessionItemStyle,
  borderColor: colors.cyan,
  boxShadow: `0 0 10px ${colors.cyan}30`,
}

export function SessionBrowser({ workerId, onSelectChunk }: SessionBrowserProps) {
  const [sessions, setSessions] = useState<CollectionSession[]>([])
  const [selectedSession, setSelectedSession] = useState<string | null>(null)
  const [chunks, setChunks] = useState<CollectionChunk[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [hoveredSession, setHoveredSession] = useState<string | null>(null)

  useEffect(() => {
    loadSessions()
  }, [workerId])

  useEffect(() => {
    if (selectedSession) {
      loadChunks(selectedSession)
    }
  }, [selectedSession])

  const loadSessions = async () => {
    try {
      const data = await api.listCollectionSessions(workerId)
      setSessions(data)
    } catch (err) {
      console.error('Failed to load sessions:', err)
    } finally {
      setIsLoading(false)
    }
  }

  const loadChunks = async (sessionId: string) => {
    try {
      const data = await api.listSessionChunks(workerId, sessionId)
      setChunks(data)
    } catch (err) {
      console.error('Failed to load chunks:', err)
    }
  }

  const formatDate = (dateStr: string | null) => {
    if (!dateStr) return 'N/A'
    return new Date(dateStr).toLocaleString()
  }

  const formatDuration = (start: string | null, end: string | null) => {
    if (!start) return 'N/A'
    const startDate = new Date(start)
    const endDate = end ? new Date(end) : new Date()
    const duration = Math.floor((endDate.getTime() - startDate.getTime()) / 1000)
    const hours = Math.floor(duration / 3600)
    const minutes = Math.floor((duration % 3600) / 60)
    return `${hours}h ${minutes}m`
  }

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'active':
        return colors.green
      case 'completed':
        return colors.cyan
      default:
        return colors.textPrimary
    }
  }

  if (isLoading) {
    return (
      <div style={{
        fontFamily: 'VT323, monospace',
        fontSize: '18px',
        color: colors.cyan,
        textAlign: 'center',
        padding: '40px',
      }}>
        Loading sessions...
      </div>
    )
  }

  if (sessions.length === 0) {
    return (
      <div style={{
        fontFamily: 'VT323, monospace',
        fontSize: '18px',
        color: colors.textPrimary,
        textAlign: 'center',
        padding: '40px',
      }}>
        No collection sessions found.
      </div>
    )
  }

  return (
    <div style={{ display: 'flex', gap: '20px' }}>
      {/* Sessions List */}
      <div style={{ flex: '1' }}>
        <div style={{
          fontFamily: 'VT323, monospace',
          fontSize: '20px',
          color: colors.cyan,
          marginBottom: '15px',
          textShadow: `0 0 10px ${colors.cyan}`,
        }}>
          SESSIONS
        </div>
        <div style={{ maxHeight: '500px', overflowY: 'auto' }}>
          {sessions.map((session) => (
            <div
              key={session.session_id}
              style={hoveredSession === session.session_id || selectedSession === session.session_id
                ? sessionItemHoverStyle
                : sessionItemStyle
              }
              onClick={() => setSelectedSession(session.session_id)}
              onMouseEnter={() => setHoveredSession(session.session_id)}
              onMouseLeave={() => setHoveredSession(null)}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <span style={{
                  fontFamily: 'VT323, monospace',
                  fontSize: '18px',
                  color: colors.cyan,
                }}>
                  {session.session_id.substring(0, 20)}...
                </span>
                <span style={{
                  fontFamily: 'VT323, monospace',
                  fontSize: '14px',
                  color: getStatusColor(session.status),
                  textTransform: 'uppercase',
                }}>
                  {session.status}
                </span>
              </div>
              <div style={{
                fontFamily: 'VT323, monospace',
                fontSize: '14px',
                color: colors.textPrimary,
                marginTop: '8px',
              }}>
                <div>Started: {formatDate(session.started_at)}</div>
                <div>Duration: {formatDuration(session.started_at, session.ended_at)}</div>
                <div>Chunks: {session.total_chunks}</div>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Chunks List */}
      {selectedSession && (
        <div style={{ flex: '1' }}>
          <div style={{
            fontFamily: 'VT323, monospace',
            fontSize: '20px',
            color: colors.cyan,
            marginBottom: '15px',
            textShadow: `0 0 10px ${colors.cyan}`,
          }}>
            CHUNKS
          </div>
          <div style={{ maxHeight: '500px', overflowY: 'auto' }}>
            {chunks.length === 0 ? (
              <div style={{
                fontFamily: 'VT323, monospace',
                fontSize: '16px',
                color: colors.textPrimary,
                padding: '20px',
              }}>
                No chunks in this session.
              </div>
            ) : (
              chunks.map((chunk) => (
                <div
                  key={chunk.chunk_id}
                  style={{
                    ...sessionItemStyle,
                    cursor: onSelectChunk ? 'pointer' : 'default',
                  }}
                  onClick={() => onSelectChunk?.(chunk)}
                >
                  <div style={{
                    fontFamily: 'VT323, monospace',
                    fontSize: '16px',
                    color: colors.cyan,
                  }}>
                    Chunk #{chunk.chunk_index}
                  </div>
                  <div style={{
                    fontFamily: 'VT323, monospace',
                    fontSize: '14px',
                    color: colors.textPrimary,
                    marginTop: '5px',
                  }}>
                    <div>Size: {chunk.size_bytes ? `${(chunk.size_bytes / 1024 / 1024).toFixed(2)} MB` : 'N/A'}</div>
                    <div>Status: <span style={{ color: getStatusColor(chunk.status) }}>{chunk.status}</span></div>
                    {chunk.encrypted && <div style={{ color: colors.green }}>Encrypted</div>}
                    {chunk.r2_path && <div style={{ color: colors.magenta }}>Uploaded to R2</div>}
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  )
}
