import { useState } from 'react'
import { colors } from '../../styles'
import { api } from '../../api/client'
import { GlowButton } from '../GlowButton'

interface EncryptionSetupProps {
  workerId: string
  onComplete?: () => void
}

const inputStyle = {
  background: colors.deep,
  border: `1px solid ${colors.cyan}40`,
  color: colors.cyan,
  fontFamily: 'VT323, monospace',
  fontSize: '18px',
  padding: '12px 16px',
  borderRadius: '4px',
  outline: 'none',
  width: '100%',
}

export function EncryptionSetup({ workerId, onComplete }: EncryptionSetupProps) {
  const [password, setPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [error, setError] = useState('')
  const [success, setSuccess] = useState(false)

  const handleSubmit = async () => {
    setError('')

    if (password.length < 8) {
      setError('Password must be at least 8 characters')
      return
    }

    if (password !== confirmPassword) {
      setError('Passwords do not match')
      return
    }

    setIsSubmitting(true)
    try {
      await api.setEncryptionKey(workerId, password)
      setSuccess(true)
      onComplete?.()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to set encryption key')
    } finally {
      setIsSubmitting(false)
    }
  }

  if (success) {
    return (
      <div style={{
        background: `${colors.surface}80`,
        border: `1px solid ${colors.green}50`,
        borderRadius: '8px',
        padding: '30px',
        textAlign: 'center',
      }}>
        <div style={{
          fontFamily: 'VT323, monospace',
          fontSize: '24px',
          color: colors.green,
          marginBottom: '15px',
          textShadow: `0 0 10px ${colors.green}`,
        }}>
          ENCRYPTION KEY SET
        </div>
        <div style={{
          fontFamily: 'VT323, monospace',
          fontSize: '16px',
          color: colors.text,
          lineHeight: '1.6',
        }}>
          <p>Your data will now be encrypted before uploading to R2.</p>
          <p style={{ color: colors.yellow, marginTop: '15px' }}>
            IMPORTANT: Remember your password! It is NOT stored on the server.
            <br />
            You will need it to decrypt your data.
          </p>
        </div>
      </div>
    )
  }

  return (
    <div style={{
      background: `${colors.surface}80`,
      border: `1px solid ${colors.cyan}30`,
      borderRadius: '8px',
      padding: '30px',
    }}>
      <div style={{
        fontFamily: 'VT323, monospace',
        fontSize: '22px',
        color: colors.cyan,
        marginBottom: '20px',
        textShadow: `0 0 10px ${colors.cyan}`,
      }}>
        SET ENCRYPTION PASSWORD
      </div>

      <div style={{
        fontFamily: 'VT323, monospace',
        fontSize: '14px',
        color: colors.text,
        marginBottom: '25px',
        lineHeight: '1.6',
      }}>
        <p>
          Enable encryption to protect your collected data before uploading to cloud storage.
          Data is encrypted using X25519 + AES-256-GCM.
        </p>
        <p style={{ color: colors.yellow, marginTop: '10px' }}>
          WARNING: The password is NOT stored on the server. If you forget it,
          your encrypted data cannot be recovered.
        </p>
      </div>

      <div style={{ marginBottom: '20px' }}>
        <label style={{
          fontFamily: 'VT323, monospace',
          fontSize: '16px',
          color: colors.text,
          display: 'block',
          marginBottom: '8px',
        }}>
          Password:
        </label>
        <input
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          style={inputStyle}
          placeholder="Enter encryption password"
        />
      </div>

      <div style={{ marginBottom: '25px' }}>
        <label style={{
          fontFamily: 'VT323, monospace',
          fontSize: '16px',
          color: colors.text,
          display: 'block',
          marginBottom: '8px',
        }}>
          Confirm Password:
        </label>
        <input
          type="password"
          value={confirmPassword}
          onChange={(e) => setConfirmPassword(e.target.value)}
          style={inputStyle}
          placeholder="Confirm encryption password"
        />
      </div>

      {error && (
        <div style={{
          fontFamily: 'VT323, monospace',
          fontSize: '16px',
          color: colors.red,
          marginBottom: '15px',
        }}>
          {error}
        </div>
      )}

      <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
        <GlowButton onClick={handleSubmit} disabled={isSubmitting}>
          {isSubmitting ? 'SETTING KEY...' : 'SET ENCRYPTION KEY'}
        </GlowButton>
      </div>
    </div>
  )
}
