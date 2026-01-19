import React, { useState, useEffect } from 'react'
import { colors } from '../styles'
import { GlowButton } from '../components/GlowButton'
import { useAuth } from '../hooks/useAuth'
import { api } from '../api/client'

const bootMessages = [
  'INITIALIZING CORELY SYSTEMS...',
  'LOADING KERNEL MODULES...',
  'ESTABLISHING SECURE CHANNEL...',
  'NEURAL INTERFACE READY...',
  'AWAITING AUTHENTICATION...',
]

export function LoginPage() {
  const { login } = useAuth()
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [verificationCode, setVerificationCode] = useState('')
  const [pendingToken, setPendingToken] = useState('')
  const [step, setStep] = useState<'credentials' | '2fa'>('credentials')
  const [error, setError] = useState('')
  const [isLoading, setIsLoading] = useState(false)
  const [bootIndex, setBootIndex] = useState(0)
  const [showLogin, setShowLogin] = useState(false)

  // Boot sequence animation
  useEffect(() => {
    if (bootIndex < bootMessages.length) {
      const timer = setTimeout(() => {
        setBootIndex(bootIndex + 1)
      }, 400)
      return () => clearTimeout(timer)
    } else {
      setTimeout(() => setShowLogin(true), 300)
    }
  }, [bootIndex])

  const handleCredentialsSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError('')
    setIsLoading(true)

    try {
      const response = await fetch('/api/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: `username=${encodeURIComponent(username)}&password=${encodeURIComponent(password)}`,
      })

      if (!response.ok) {
        throw new Error('Invalid credentials')
      }

      const data = await response.json()

      if (data.requires_2fa) {
        setPendingToken(data.pending_token)
        setStep('2fa')
      } else {
        // Old flow (direct token)
        await login(username, password)
      }
    } catch (err) {
      setError('ACCESS DENIED: Invalid credentials')
    } finally {
      setIsLoading(false)
    }
  }

  const handleVerificationSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError('')
    setIsLoading(true)

    try {
      const response = await fetch('/api/auth/verify', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ pending_token: pendingToken, code: verificationCode }),
      })

      if (!response.ok) {
        throw new Error('Invalid verification code')
      }

      const data = await response.json()
      // Store token using the API client and redirect
      api.setToken(data.access_token)
      window.location.href = '/'
    } catch (err) {
      setError('ACCESS DENIED: Invalid verification code')
    } finally {
      setIsLoading(false)
    }
  }

  return (
    <div style={{
      minHeight: '100vh',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      background: `
        radial-gradient(ellipse at center, ${colors.surface} 0%, ${colors.void} 100%),
        repeating-linear-gradient(
          0deg,
          transparent,
          transparent 2px,
          rgba(0, 255, 255, 0.03) 2px,
          rgba(0, 255, 255, 0.03) 4px
        )
      `,
      padding: 20,
    }}>
      {/* Background grid */}
      <div style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        backgroundImage: `
          linear-gradient(${colors.cyan}11 1px, transparent 1px),
          linear-gradient(90deg, ${colors.cyan}11 1px, transparent 1px)
        `,
        backgroundSize: '50px 50px',
        pointerEvents: 'none',
      }} />

      <div style={{
        width: '100%',
        maxWidth: 480,
        position: 'relative',
      }}>
        {/* Logo / Title */}
        <div style={{
          textAlign: 'center',
          marginBottom: 40,
        }}>
          <div style={{
            fontFamily: 'VT323, monospace',
            fontSize: '64px',
            color: colors.cyan,
            textShadow: colors.cyanGlow,
            letterSpacing: '8px',
            animation: 'flicker 4s infinite',
          }}>
            CORELY
          </div>
          <div style={{
            fontSize: '12px',
            color: colors.textMuted,
            letterSpacing: '4px',
            marginTop: 8,
          }}>
            REMOTE SYSTEMS INTERFACE v0.1.0
          </div>
        </div>

        {/* Boot sequence */}
        <div style={{
          background: colors.deep,
          border: `1px solid ${colors.cyan}`,
          padding: 24,
          marginBottom: 24,
          fontFamily: 'Share Tech Mono, monospace',
          fontSize: '13px',
          minHeight: 180,
        }}>
          {bootMessages.slice(0, bootIndex).map((msg, i) => (
            <div
              key={i}
              style={{
                color: i === bootIndex - 1 ? colors.cyan : colors.green,
                marginBottom: 8,
                animation: 'bootSequence 0.3s ease',
              }}
            >
              <span style={{ color: colors.textMuted }}>[{String(i + 1).padStart(2, '0')}]</span>{' '}
              {msg}
              {i === bootIndex - 1 && <span style={{ animation: 'blink 1s infinite' }}>_</span>}
            </div>
          ))}

          {bootIndex >= bootMessages.length && (
            <div style={{
              color: colors.green,
              marginTop: 16,
              animation: 'bootSequence 0.3s ease',
            }}>
              <span style={{ color: colors.textMuted }}>[OK]</span>{' '}
              SYSTEM READY
            </div>
          )}
        </div>

        {/* Login form */}
        {showLogin && step === 'credentials' && (
          <form onSubmit={handleCredentialsSubmit} style={{ animation: 'bootSequence 0.5s ease' }}>
            <div style={{
              background: colors.deep,
              border: `1px solid ${colors.cyan}`,
              padding: 32,
              position: 'relative',
            }}>
              {/* Corner brackets */}
              <div style={{
                position: 'absolute', top: -1, left: -1,
                width: 20, height: 20,
                borderTop: `2px solid ${colors.magenta}`,
                borderLeft: `2px solid ${colors.magenta}`,
              }} />
              <div style={{
                position: 'absolute', top: -1, right: -1,
                width: 20, height: 20,
                borderTop: `2px solid ${colors.magenta}`,
                borderRight: `2px solid ${colors.magenta}`,
              }} />
              <div style={{
                position: 'absolute', bottom: -1, left: -1,
                width: 20, height: 20,
                borderBottom: `2px solid ${colors.magenta}`,
                borderLeft: `2px solid ${colors.magenta}`,
              }} />
              <div style={{
                position: 'absolute', bottom: -1, right: -1,
                width: 20, height: 20,
                borderBottom: `2px solid ${colors.magenta}`,
                borderRight: `2px solid ${colors.magenta}`,
              }} />

              <div style={{
                fontFamily: 'VT323, monospace',
                fontSize: '18px',
                color: colors.magenta,
                textShadow: colors.magentaGlow,
                marginBottom: 24,
                letterSpacing: '2px',
              }}>
                [ AUTHENTICATION REQUIRED ]
              </div>

              {error && (
                <div style={{
                  background: `${colors.red}22`,
                  border: `1px solid ${colors.red}`,
                  padding: '12px 16px',
                  marginBottom: 20,
                  color: colors.red,
                  fontSize: '12px',
                  textShadow: colors.redGlow,
                }}>
                  {error}
                </div>
              )}

              <div style={{ marginBottom: 20 }}>
                <label style={{
                  display: 'block',
                  fontSize: '11px',
                  color: colors.textMuted,
                  marginBottom: 8,
                  letterSpacing: '2px',
                }}>
                  USER_ID
                </label>
                <input
                  type="text"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  style={{
                    width: '100%',
                    padding: '12px 16px',
                    background: colors.surface,
                    border: `1px solid ${colors.textMuted}`,
                    color: colors.cyan,
                    fontFamily: 'Share Tech Mono, monospace',
                    fontSize: '14px',
                    outline: 'none',
                    transition: 'all 0.3s ease',
                  }}
                  onFocus={(e) => {
                    e.target.style.borderColor = colors.cyan
                    e.target.style.boxShadow = `0 0 10px ${colors.cyan}44`
                  }}
                  onBlur={(e) => {
                    e.target.style.borderColor = colors.textMuted
                    e.target.style.boxShadow = 'none'
                  }}
                />
              </div>

              <div style={{ marginBottom: 28 }}>
                <label style={{
                  display: 'block',
                  fontSize: '11px',
                  color: colors.textMuted,
                  marginBottom: 8,
                  letterSpacing: '2px',
                }}>
                  ACCESS_KEY
                </label>
                <input
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  style={{
                    width: '100%',
                    padding: '12px 16px',
                    background: colors.surface,
                    border: `1px solid ${colors.textMuted}`,
                    color: colors.cyan,
                    fontFamily: 'Share Tech Mono, monospace',
                    fontSize: '14px',
                    outline: 'none',
                    transition: 'all 0.3s ease',
                  }}
                  onFocus={(e) => {
                    e.target.style.borderColor = colors.cyan
                    e.target.style.boxShadow = `0 0 10px ${colors.cyan}44`
                  }}
                  onBlur={(e) => {
                    e.target.style.borderColor = colors.textMuted
                    e.target.style.boxShadow = 'none'
                  }}
                />
              </div>

              <GlowButton
                type="submit"
                disabled={isLoading || !username || !password}
                variant="magenta"
                style={{ width: '100%' }}
              >
                {isLoading ? '◐ AUTHENTICATING...' : '► INITIALIZE SESSION'}
              </GlowButton>
            </div>
          </form>
        )}

        {/* 2FA Verification form */}
        {showLogin && step === '2fa' && (
          <form onSubmit={handleVerificationSubmit} style={{ animation: 'bootSequence 0.5s ease' }}>
            <div style={{
              background: colors.deep,
              border: `1px solid ${colors.green}`,
              padding: 32,
              position: 'relative',
            }}>
              {/* Corner brackets */}
              <div style={{
                position: 'absolute', top: -1, left: -1,
                width: 20, height: 20,
                borderTop: `2px solid ${colors.green}`,
                borderLeft: `2px solid ${colors.green}`,
              }} />
              <div style={{
                position: 'absolute', top: -1, right: -1,
                width: 20, height: 20,
                borderTop: `2px solid ${colors.green}`,
                borderRight: `2px solid ${colors.green}`,
              }} />
              <div style={{
                position: 'absolute', bottom: -1, left: -1,
                width: 20, height: 20,
                borderBottom: `2px solid ${colors.green}`,
                borderLeft: `2px solid ${colors.green}`,
              }} />
              <div style={{
                position: 'absolute', bottom: -1, right: -1,
                width: 20, height: 20,
                borderBottom: `2px solid ${colors.green}`,
                borderRight: `2px solid ${colors.green}`,
              }} />

              <div style={{
                fontFamily: 'VT323, monospace',
                fontSize: '18px',
                color: colors.green,
                textShadow: colors.greenGlow,
                marginBottom: 24,
                letterSpacing: '2px',
              }}>
                [ SECONDARY VERIFICATION ]
              </div>

              {error && (
                <div style={{
                  background: `${colors.red}22`,
                  border: `1px solid ${colors.red}`,
                  padding: '12px 16px',
                  marginBottom: 20,
                  color: colors.red,
                  fontSize: '12px',
                  textShadow: colors.redGlow,
                }}>
                  {error}
                </div>
              )}

              <div style={{
                fontSize: '12px',
                color: colors.textMuted,
                marginBottom: 20,
                lineHeight: 1.6,
              }}>
                Enter your verification code to complete authentication.
              </div>

              <div style={{ marginBottom: 28 }}>
                <label style={{
                  display: 'block',
                  fontSize: '11px',
                  color: colors.textMuted,
                  marginBottom: 8,
                  letterSpacing: '2px',
                }}>
                  VERIFICATION_CODE
                </label>
                <input
                  type="password"
                  value={verificationCode}
                  onChange={(e) => setVerificationCode(e.target.value)}
                  autoFocus
                  style={{
                    width: '100%',
                    padding: '12px 16px',
                    background: colors.surface,
                    border: `1px solid ${colors.textMuted}`,
                    color: colors.green,
                    fontFamily: 'Share Tech Mono, monospace',
                    fontSize: '18px',
                    letterSpacing: '4px',
                    outline: 'none',
                    transition: 'all 0.3s ease',
                  }}
                  onFocus={(e) => {
                    e.target.style.borderColor = colors.green
                    e.target.style.boxShadow = `0 0 10px ${colors.green}44`
                  }}
                  onBlur={(e) => {
                    e.target.style.borderColor = colors.textMuted
                    e.target.style.boxShadow = 'none'
                  }}
                />
              </div>

              <GlowButton
                type="submit"
                disabled={isLoading || !verificationCode}
                variant="green"
                style={{ width: '100%' }}
              >
                {isLoading ? '◐ VERIFYING...' : '► COMPLETE AUTHENTICATION'}
              </GlowButton>
            </div>
          </form>
        )}

        {/* Footer */}
        <div style={{
          textAlign: 'center',
          marginTop: 32,
          fontSize: '10px',
          color: colors.textMuted,
          letterSpacing: '1px',
        }}>
          SECURE CHANNEL ESTABLISHED • PROTOCOL v2.0 • {new Date().getFullYear()}
        </div>
      </div>
    </div>
  )
}
