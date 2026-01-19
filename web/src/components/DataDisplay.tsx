import { colors } from '../styles'

interface DataRowProps {
  label: string
  value: string | number
  color?: string
  unit?: string
}

export function DataRow({ label, value, color = colors.cyan, unit }: DataRowProps) {
  return (
    <div style={{
      display: 'flex',
      justifyContent: 'space-between',
      alignItems: 'center',
      padding: '8px 0',
      borderBottom: `1px solid ${colors.panel}`,
    }}>
      <span style={{
        color: colors.textMuted,
        fontSize: '12px',
        textTransform: 'uppercase',
        letterSpacing: '1px',
      }}>
        {label}
      </span>
      <span style={{
        color: color,
        fontFamily: 'VT323, monospace',
        fontSize: '16px',
        textShadow: `0 0 5px ${color}`,
      }}>
        {value}{unit && <span style={{ fontSize: '12px', opacity: 0.7 }}> {unit}</span>}
      </span>
    </div>
  )
}

interface ProgressBarProps {
  value: number
  max?: number
  label?: string
  color?: string
  showValue?: boolean
}

export function ProgressBar({ value, max = 100, label, color = colors.cyan, showValue = true }: ProgressBarProps) {
  const percentage = Math.min((value / max) * 100, 100)

  return (
    <div style={{ marginBottom: 12 }}>
      {label && (
        <div style={{
          display: 'flex',
          justifyContent: 'space-between',
          marginBottom: 6,
        }}>
          <span style={{
            color: colors.textMuted,
            fontSize: '11px',
            textTransform: 'uppercase',
            letterSpacing: '1px',
          }}>
            {label}
          </span>
          {showValue && (
            <span style={{
              color: color,
              fontFamily: 'VT323, monospace',
              fontSize: '14px',
            }}>
              {percentage.toFixed(1)}%
            </span>
          )}
        </div>
      )}
      <div style={{
        height: 8,
        background: colors.panel,
        border: `1px solid ${colors.textMuted}`,
        position: 'relative',
        overflow: 'hidden',
      }}>
        <div style={{
          position: 'absolute',
          top: 0,
          left: 0,
          height: '100%',
          width: `${percentage}%`,
          background: `linear-gradient(90deg, ${color}, ${color}88)`,
          boxShadow: `0 0 10px ${color}`,
          transition: 'width 0.5s ease',
        }} />
        {/* Segment lines */}
        {[...Array(10)].map((_, i) => (
          <div
            key={i}
            style={{
              position: 'absolute',
              top: 0,
              left: `${(i + 1) * 10}%`,
              height: '100%',
              width: 1,
              background: colors.deep,
            }}
          />
        ))}
      </div>
    </div>
  )
}

interface StatBlockProps {
  icon?: string
  label: string
  value: string | number
  subValue?: string
  color?: string
}

export function StatBlock({ icon, label, value, subValue, color = colors.cyan }: StatBlockProps) {
  return (
    <div style={{
      background: colors.panel,
      border: `1px solid ${colors.textMuted}`,
      padding: 16,
      textAlign: 'center',
      position: 'relative',
    }}>
      {/* Corner accent */}
      <div style={{
        position: 'absolute',
        top: 0,
        left: 0,
        width: 8,
        height: 8,
        borderTop: `2px solid ${color}`,
        borderLeft: `2px solid ${color}`,
      }} />
      <div style={{
        position: 'absolute',
        bottom: 0,
        right: 0,
        width: 8,
        height: 8,
        borderBottom: `2px solid ${color}`,
        borderRight: `2px solid ${color}`,
      }} />

      {icon && (
        <div style={{
          fontSize: '24px',
          marginBottom: 8,
        }}>
          {icon}
        </div>
      )}
      <div style={{
        color: color,
        fontFamily: 'VT323, monospace',
        fontSize: '28px',
        textShadow: `0 0 10px ${color}`,
        marginBottom: 4,
      }}>
        {value}
      </div>
      <div style={{
        color: colors.textMuted,
        fontSize: '10px',
        textTransform: 'uppercase',
        letterSpacing: '2px',
      }}>
        {label}
      </div>
      {subValue && (
        <div style={{
          color: colors.textSecondary,
          fontSize: '11px',
          marginTop: 4,
        }}>
          {subValue}
        </div>
      )}
    </div>
  )
}
