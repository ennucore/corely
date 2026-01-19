import React from 'react'
import { colors } from '../styles'

interface CRTFrameProps {
  children: React.ReactNode
  title?: string
  subtitle?: string
  status?: 'online' | 'offline' | 'warning'
  style?: React.CSSProperties
}

export function CRTFrame({ children, title, subtitle, status, style }: CRTFrameProps) {
  const statusColor = status === 'online' ? colors.green : status === 'warning' ? colors.amber : colors.red

  return (
    <div style={{
      background: colors.deep,
      border: `1px solid ${colors.cyan}`,
      boxShadow: `inset 0 0 30px rgba(0, 255, 255, 0.03), 0 0 10px rgba(0, 255, 255, 0.1)`,
      position: 'relative',
      ...style,
    }}>
      {/* Corner brackets */}
      <div style={{
        position: 'absolute',
        top: -1,
        left: -1,
        width: 20,
        height: 20,
        borderTop: `2px solid ${colors.cyan}`,
        borderLeft: `2px solid ${colors.cyan}`,
      }} />
      <div style={{
        position: 'absolute',
        top: -1,
        right: -1,
        width: 20,
        height: 20,
        borderTop: `2px solid ${colors.cyan}`,
        borderRight: `2px solid ${colors.cyan}`,
      }} />
      <div style={{
        position: 'absolute',
        bottom: -1,
        left: -1,
        width: 20,
        height: 20,
        borderBottom: `2px solid ${colors.cyan}`,
        borderLeft: `2px solid ${colors.cyan}`,
      }} />
      <div style={{
        position: 'absolute',
        bottom: -1,
        right: -1,
        width: 20,
        height: 20,
        borderBottom: `2px solid ${colors.cyan}`,
        borderRight: `2px solid ${colors.cyan}`,
      }} />

      {/* Header */}
      {(title || status) && (
        <div style={{
          padding: '12px 16px',
          borderBottom: `1px solid ${colors.panel}`,
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          background: `linear-gradient(90deg, rgba(0, 255, 255, 0.05) 0%, transparent 100%)`,
        }}>
          <div>
            {title && (
              <div style={{
                fontFamily: 'VT323, monospace',
                fontSize: '18px',
                color: colors.cyan,
                textTransform: 'uppercase',
                letterSpacing: '2px',
                textShadow: `0 0 10px ${colors.cyan}`,
              }}>
                [&nbsp;{title}&nbsp;]
              </div>
            )}
            {subtitle && (
              <div style={{
                fontSize: '11px',
                color: colors.textMuted,
                marginTop: 4,
              }}>
                {subtitle}
              </div>
            )}
          </div>
          {status && (
            <div style={{
              display: 'flex',
              alignItems: 'center',
              gap: 8,
            }}>
              <div style={{
                width: 8,
                height: 8,
                borderRadius: '50%',
                background: statusColor,
                boxShadow: `0 0 10px ${statusColor}`,
                animation: status === 'online' ? 'pulse 2s infinite' : 'none',
              }} />
              <span style={{
                fontSize: '11px',
                color: statusColor,
                textTransform: 'uppercase',
                letterSpacing: '1px',
              }}>
                {status}
              </span>
            </div>
          )}
        </div>
      )}

      {/* Content */}
      <div style={{ padding: 16 }}>
        {children}
      </div>
    </div>
  )
}
