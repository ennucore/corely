import React, { useState } from 'react'
import { colors } from '../styles'

interface GlowButtonProps {
  children: React.ReactNode
  onClick?: () => void
  disabled?: boolean
  variant?: 'cyan' | 'magenta' | 'green' | 'red'
  size?: 'small' | 'medium' | 'large'
  style?: React.CSSProperties
  type?: 'button' | 'submit'
}

export function GlowButton({
  children,
  onClick,
  disabled = false,
  variant = 'cyan',
  size = 'medium',
  style,
  type = 'button',
}: GlowButtonProps) {
  const [isHovered, setIsHovered] = useState(false)

  const variantColors = {
    cyan: colors.cyan,
    magenta: colors.magenta,
    green: colors.green,
    red: colors.red,
  }

  const color = variantColors[variant]

  const sizes = {
    small: { padding: '6px 12px', fontSize: '12px' },
    medium: { padding: '10px 20px', fontSize: '14px' },
    large: { padding: '14px 28px', fontSize: '16px' },
  }

  return (
    <button
      type={type}
      onClick={onClick}
      disabled={disabled}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      style={{
        fontFamily: 'VT323, monospace',
        background: isHovered && !disabled ? color : 'transparent',
        color: isHovered && !disabled ? colors.void : color,
        border: `1px solid ${color}`,
        cursor: disabled ? 'not-allowed' : 'pointer',
        textTransform: 'uppercase',
        letterSpacing: '2px',
        transition: 'all 0.2s ease',
        boxShadow: isHovered && !disabled ? `0 0 20px ${color}, inset 0 0 20px ${color}` : `0 0 5px ${color}`,
        opacity: disabled ? 0.5 : 1,
        position: 'relative',
        overflow: 'hidden',
        ...sizes[size],
        ...style,
      }}
    >
      {/* Scan line effect on hover */}
      {isHovered && !disabled && (
        <div style={{
          position: 'absolute',
          top: 0,
          left: 0,
          right: 0,
          height: '2px',
          background: `linear-gradient(90deg, transparent, ${colors.void}, transparent)`,
          animation: 'scanline 0.5s linear infinite',
        }} />
      )}
      {children}
    </button>
  )
}
