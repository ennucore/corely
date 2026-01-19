import { useState, useEffect } from 'react'
import { colors } from '../styles'
import { api } from '../api/client'
import type { Worker } from '../api/client'

interface WorkerCardProps {
  worker: Worker
  onClick: () => void
}

export function WorkerCard({ worker, onClick }: WorkerCardProps) {
  const [isHovered, setIsHovered] = useState(false)
  const [screenshot, setScreenshot] = useState<string | null>(null)
  const [screenshotLoading, setScreenshotLoading] = useState(false)

  const statusColor = worker.is_online ? colors.green : colors.red
  const osIcon = worker.os?.toLowerCase().includes('mac') ? '🍎' :
                 worker.os?.toLowerCase().includes('windows') ? '🪟' :
                 worker.os?.toLowerCase().includes('linux') ? '🐧' : '💻'

  // Fetch screenshot for online workers
  useEffect(() => {
    if (worker.is_online && !screenshot && !screenshotLoading) {
      setScreenshotLoading(true)
      api.getScreen(worker.id)
        .then(data => {
          setScreenshot(`data:image/${data.format};base64,${data.data}`)
        })
        .catch(() => {
          // Silently fail - screenshot not available
        })
        .finally(() => {
          setScreenshotLoading(false)
        })
    }
  }, [worker.id, worker.is_online])

  return (
    <div
      onClick={onClick}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      style={{
        background: isHovered ? colors.panel : colors.surface,
        border: `1px solid ${isHovered ? colors.cyan : colors.textMuted}`,
        cursor: 'pointer',
        transition: 'all 0.3s ease',
        position: 'relative',
        overflow: 'hidden',
        boxShadow: isHovered ? `0 0 20px rgba(0, 255, 255, 0.2), inset 0 0 30px rgba(0, 255, 255, 0.05)` : 'none',
      }}
    >
      {/* Screenshot preview */}
      <div style={{
        height: 140,
        background: colors.void,
        position: 'relative',
        overflow: 'hidden',
        borderBottom: `1px solid ${colors.panel}`,
      }}>
        {screenshot ? (
          <img
            src={screenshot}
            alt={`${worker.name} screen`}
            style={{
              width: '100%',
              height: '100%',
              objectFit: 'cover',
              opacity: isHovered ? 1 : 0.8,
              transition: 'opacity 0.3s ease',
            }}
          />
        ) : (
          <div style={{
            width: '100%',
            height: '100%',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            flexDirection: 'column',
            gap: 8,
          }}>
            {screenshotLoading ? (
              <div style={{
                color: colors.cyan,
                fontFamily: 'VT323, monospace',
                fontSize: '14px',
                animation: 'pulse 1s infinite',
              }}>
                LOADING...
              </div>
            ) : worker.is_online ? (
              <div style={{ color: colors.textMuted, fontSize: '12px' }}>
                No preview
              </div>
            ) : (
              <>
                <div style={{ fontSize: '24px', opacity: 0.3 }}>📺</div>
                <div style={{ color: colors.textMuted, fontSize: '10px', textTransform: 'uppercase' }}>
                  Offline
                </div>
              </>
            )}
          </div>
        )}

        {/* CRT overlay effect */}
        <div style={{
          position: 'absolute',
          top: 0,
          left: 0,
          right: 0,
          bottom: 0,
          background: 'repeating-linear-gradient(0deg, rgba(0,0,0,0.1) 0px, rgba(0,0,0,0.1) 1px, transparent 1px, transparent 2px)',
          pointerEvents: 'none',
        }} />
      </div>

      {/* Scan line effect on hover */}
      {isHovered && (
        <div style={{
          position: 'absolute',
          top: 0,
          left: 0,
          right: 0,
          bottom: 0,
          background: `linear-gradient(180deg, transparent 0%, rgba(0, 255, 255, 0.02) 50%, transparent 100%)`,
          animation: 'scanline 2s linear infinite',
          pointerEvents: 'none',
        }} />
      )}

      {/* Status indicator strip */}
      <div style={{
        height: 3,
        background: statusColor,
        boxShadow: `0 0 10px ${statusColor}`,
      }} />

      <div style={{ padding: 16 }}>
        {/* Header */}
        <div style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'flex-start',
          marginBottom: 12,
        }}>
          <div>
            <div style={{
              fontFamily: 'VT323, monospace',
              fontSize: '20px',
              color: isHovered ? colors.cyan : colors.textPrimary,
              textShadow: isHovered ? `0 0 10px ${colors.cyan}` : 'none',
              transition: 'all 0.3s ease',
            }}>
              {osIcon} {worker.name}
            </div>
            <div style={{
              fontSize: '11px',
              color: colors.textMuted,
              fontFamily: 'Share Tech Mono, monospace',
            }}>
              ID: {worker.id.slice(0, 8)}...
            </div>
          </div>

          <div style={{
            display: 'flex',
            alignItems: 'center',
            gap: 6,
          }}>
            <div style={{
              width: 8,
              height: 8,
              borderRadius: '50%',
              background: statusColor,
              boxShadow: `0 0 10px ${statusColor}`,
              animation: worker.is_online ? 'pulse 2s infinite' : 'none',
            }} />
            <span style={{
              fontSize: '10px',
              color: statusColor,
              textTransform: 'uppercase',
              letterSpacing: '1px',
            }}>
              {worker.is_online ? 'ONLINE' : 'OFFLINE'}
            </span>
          </div>
        </div>

        {/* Info grid */}
        <div style={{
          display: 'grid',
          gridTemplateColumns: '1fr 1fr',
          gap: 8,
          fontSize: '11px',
        }}>
          <div>
            <span style={{ color: colors.textMuted }}>HOST: </span>
            <span style={{ color: colors.textSecondary }}>{worker.hostname || 'N/A'}</span>
          </div>
          <div>
            <span style={{ color: colors.textMuted }}>OS: </span>
            <span style={{ color: colors.textSecondary }}>{worker.os || 'N/A'}</span>
          </div>
          <div>
            <span style={{ color: colors.textMuted }}>ARCH: </span>
            <span style={{ color: colors.textSecondary }}>{worker.arch || 'N/A'}</span>
          </div>
          <div>
            <span style={{ color: colors.textMuted }}>CAPS: </span>
            <span style={{ color: colors.textSecondary }}>{worker.capabilities?.length || 0}</span>
          </div>
        </div>

        {/* Last seen */}
        {worker.last_seen && (
          <div style={{
            marginTop: 12,
            paddingTop: 12,
            borderTop: `1px solid ${colors.panel}`,
            fontSize: '10px',
            color: colors.textMuted,
          }}>
            LAST SEEN: {new Date(worker.last_seen).toLocaleString()}
          </div>
        )}
      </div>

      {/* Corner brackets */}
      <div style={{
        position: 'absolute',
        bottom: 4,
        right: 4,
        width: 12,
        height: 12,
        borderBottom: `2px solid ${isHovered ? colors.cyan : colors.textMuted}`,
        borderRight: `2px solid ${isHovered ? colors.cyan : colors.textMuted}`,
        transition: 'all 0.3s ease',
      }} />
    </div>
  )
}
