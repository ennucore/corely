import { useState, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { colors } from '../styles'
import { api, OAuthClient, getServerBaseUrl } from '../api/client'
import { CRTFrame } from '../components/CRTFrame'
import { GlowButton } from '../components/GlowButton'
import { useAuth } from '../hooks/useAuth'

export function SettingsPage() {
  const navigate = useNavigate()
  const { user, logout, setToken } = useAuth()
  const [clients, setClients] = useState<OAuthClient[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState('')
  const [success, setSuccess] = useState('')
  const [showCreateForm, setShowCreateForm] = useState(false)
  const [newClientName, setNewClientName] = useState('')
  const [createdClient, setCreatedClient] = useState<OAuthClient | null>(null)
  const [copied, setCopied] = useState<string | null>(null)

  // Account settings
  const [currentPassword, setCurrentPassword] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [newUsername, setNewUsername] = useState('')
  const [isChangingPassword, setIsChangingPassword] = useState(false)
  const [isChangingUsername, setIsChangingUsername] = useState(false)

  const mcpUrl = api.getMcpUrl()

  useEffect(() => {
    fetchClients()
  }, [])

  const fetchClients = async () => {
    try {
      const data = await api.listOAuthClients()
      setClients(data)
      setError('')
    } catch (err) {
      setError('Failed to fetch OAuth clients')
    } finally {
      setIsLoading(false)
    }
  }

  const createClient = async () => {
    if (!newClientName.trim()) return

    try {
      const client = await api.createOAuthClient(newClientName.trim())
      setCreatedClient(client)
      setNewClientName('')
      setShowCreateForm(false)
      fetchClients()
    } catch (err) {
      setError('Failed to create OAuth client')
    }
  }

  const revokeClient = async (clientId: string) => {
    if (!confirm('Revoke this OAuth client? This cannot be undone.')) return

    try {
      await api.revokeOAuthClient(clientId)
      fetchClients()
    } catch (err) {
      setError('Failed to revoke OAuth client')
    }
  }

  const copyToClipboard = (text: string, key: string) => {
    navigator.clipboard.writeText(text)
    setCopied(key)
    setTimeout(() => setCopied(null), 2000)
  }

  const handleChangePassword = async () => {
    if (newPassword !== confirmPassword) {
      setError('Passwords do not match')
      return
    }
    if (newPassword.length < 4) {
      setError('Password must be at least 4 characters')
      return
    }

    setIsChangingPassword(true)
    setError('')
    setSuccess('')

    try {
      await api.changePassword(currentPassword, newPassword)
      setSuccess('Password changed successfully')
      setCurrentPassword('')
      setNewPassword('')
      setConfirmPassword('')
    } catch (err: any) {
      setError(err.message || 'Failed to change password')
    } finally {
      setIsChangingPassword(false)
    }
  }

  const handleChangeUsername = async () => {
    if (!newUsername.trim()) {
      setError('Username cannot be empty')
      return
    }

    setIsChangingUsername(true)
    setError('')
    setSuccess('')

    try {
      const result = await api.changeUsername(newUsername.trim())
      setToken(result.access_token)
      setSuccess(`Username changed to ${result.new_username}`)
      setNewUsername('')
    } catch (err: any) {
      setError(err.message || 'Failed to change username')
    } finally {
      setIsChangingUsername(false)
    }
  }

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
          <div
            onClick={() => navigate('/')}
            style={{
              fontFamily: 'VT323, monospace',
              fontSize: '28px',
              color: colors.cyan,
              textShadow: colors.cyanGlow,
              letterSpacing: '4px',
              cursor: 'pointer',
            }}
          >
            CORELY
          </div>
          <div style={{
            fontSize: '11px',
            color: colors.textMuted,
            borderLeft: `1px solid ${colors.panel}`,
            paddingLeft: 24,
          }}>
            SETTINGS / MCP ACCESS
          </div>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 24 }}>
          <div style={{
            fontSize: '11px',
            color: colors.textSecondary,
          }}>
            <span style={{ color: colors.textMuted }}>USER:</span> {user?.username.toUpperCase()}
          </div>

          <GlowButton size="small" variant="cyan" onClick={() => navigate('/')}>
            DASHBOARD
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
        maxWidth: 1000,
        margin: '0 auto',
        width: '100%',
      }}>
        {/* MCP URL Section */}
        <CRTFrame
          title="MCP ENDPOINT"
          subtitle="Server-Sent Events URL for Claude Code integration"
          status="online"
          style={{ marginBottom: 24 }}
        >
          <div style={{ padding: 16 }}>
            <div style={{
              display: 'flex',
              alignItems: 'center',
              gap: 12,
              marginBottom: 16,
            }}>
              <code style={{
                flex: 1,
                fontFamily: 'Share Tech Mono, monospace',
                fontSize: '13px',
                color: colors.green,
                background: colors.void,
                padding: '10px 14px',
                border: `1px solid ${colors.cyan}`,
              }}>
                {mcpUrl}
              </code>
              <GlowButton
                size="small"
                variant="cyan"
                onClick={() => copyToClipboard(mcpUrl, 'mcp-url')}
              >
                {copied === 'mcp-url' ? '✓' : '📋'}
              </GlowButton>
            </div>

            <div style={{
              fontSize: '11px',
              color: colors.textMuted,
              lineHeight: 1.8,
            }}>
              <p style={{ marginBottom: 8 }}>
                To use Corely with Claude Code, create an OAuth client below and add to your settings:
              </p>
              <pre style={{
                background: colors.panel,
                padding: 12,
                border: `1px solid ${colors.textMuted}`,
                overflow: 'auto',
                color: colors.textSecondary,
              }}>
{`{
  "mcpServers": {
    "corely": {
      "url": "${mcpUrl}",
      "headers": {
        "Authorization": "Bearer <access_token>"
      }
    }
  }
}`}
              </pre>
            </div>
          </div>
        </CRTFrame>

        {/* Account Settings */}
        <CRTFrame
          title="ACCOUNT SETTINGS"
          subtitle="Change username and password"
          status="online"
          style={{ marginBottom: 24 }}
        >
          <div style={{ padding: 16, display: 'grid', gap: 24 }}>
            {/* Change Username */}
            <div>
              <h3 style={{
                fontFamily: 'VT323, monospace',
                fontSize: '16px',
                color: colors.cyan,
                marginBottom: 12,
              }}>
                CHANGE USERNAME
              </h3>
              <div style={{
                display: 'flex',
                alignItems: 'center',
                gap: 12,
              }}>
                <input
                  type="text"
                  placeholder={`Current: ${user?.username || 'unknown'}`}
                  value={newUsername}
                  onChange={(e) => setNewUsername(e.target.value)}
                  style={{
                    flex: 1,
                    background: colors.void,
                    border: `1px solid ${colors.textMuted}`,
                    color: colors.textPrimary,
                    padding: '10px 14px',
                    fontFamily: 'Share Tech Mono, monospace',
                    fontSize: '13px',
                  }}
                />
                <GlowButton
                  size="small"
                  variant="cyan"
                  onClick={handleChangeUsername}
                  disabled={isChangingUsername || !newUsername.trim()}
                >
                  {isChangingUsername ? 'CHANGING...' : 'UPDATE'}
                </GlowButton>
              </div>
            </div>

            {/* Change Password */}
            <div>
              <h3 style={{
                fontFamily: 'VT323, monospace',
                fontSize: '16px',
                color: colors.cyan,
                marginBottom: 12,
              }}>
                CHANGE PASSWORD
              </h3>
              <div style={{
                display: 'grid',
                gap: 12,
              }}>
                <input
                  type="password"
                  placeholder="Current password"
                  value={currentPassword}
                  onChange={(e) => setCurrentPassword(e.target.value)}
                  style={{
                    background: colors.void,
                    border: `1px solid ${colors.textMuted}`,
                    color: colors.textPrimary,
                    padding: '10px 14px',
                    fontFamily: 'Share Tech Mono, monospace',
                    fontSize: '13px',
                  }}
                />
                <input
                  type="password"
                  placeholder="New password"
                  value={newPassword}
                  onChange={(e) => setNewPassword(e.target.value)}
                  style={{
                    background: colors.void,
                    border: `1px solid ${colors.textMuted}`,
                    color: colors.textPrimary,
                    padding: '10px 14px',
                    fontFamily: 'Share Tech Mono, monospace',
                    fontSize: '13px',
                  }}
                />
                <input
                  type="password"
                  placeholder="Confirm new password"
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                  style={{
                    background: colors.void,
                    border: `1px solid ${colors.textMuted}`,
                    color: colors.textPrimary,
                    padding: '10px 14px',
                    fontFamily: 'Share Tech Mono, monospace',
                    fontSize: '13px',
                  }}
                />
                <div>
                  <GlowButton
                    size="small"
                    variant="green"
                    onClick={handleChangePassword}
                    disabled={isChangingPassword || !currentPassword || !newPassword || !confirmPassword}
                  >
                    {isChangingPassword ? 'CHANGING...' : 'CHANGE PASSWORD'}
                  </GlowButton>
                </div>
              </div>
            </div>
          </div>
        </CRTFrame>

        {/* Success display */}
        {success && (
          <div style={{
            background: `${colors.green}22`,
            border: `1px solid ${colors.green}`,
            padding: '16px 20px',
            marginBottom: 24,
            color: colors.green,
            fontSize: '13px',
          }}>
            {success}
          </div>
        )}

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
            {error}
          </div>
        )}

        {/* Created client notification */}
        {createdClient && (
          <CRTFrame
            title="NEW CLIENT CREATED"
            subtitle="Save these credentials - the secret won't be shown again!"
            status="online"
            style={{ marginBottom: 24 }}
          >
            <div style={{ padding: 16 }}>
              <div style={{
                display: 'grid',
                gap: 12,
                marginBottom: 16,
              }}>
                <div>
                  <label style={{ color: colors.textMuted, fontSize: '10px', textTransform: 'uppercase' }}>
                    Client ID
                  </label>
                  <div style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 8,
                    marginTop: 4,
                  }}>
                    <code style={{
                      flex: 1,
                      fontFamily: 'Share Tech Mono, monospace',
                      fontSize: '12px',
                      color: colors.cyan,
                      background: colors.void,
                      padding: '8px 12px',
                      border: `1px solid ${colors.panel}`,
                    }}>
                      {createdClient.client_id}
                    </code>
                    <GlowButton
                      size="small"
                      variant="cyan"
                      onClick={() => copyToClipboard(createdClient.client_id, 'client-id')}
                    >
                      {copied === 'client-id' ? '✓' : '📋'}
                    </GlowButton>
                  </div>
                </div>

                <div>
                  <label style={{ color: colors.textMuted, fontSize: '10px', textTransform: 'uppercase' }}>
                    Client Secret
                  </label>
                  <div style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 8,
                    marginTop: 4,
                  }}>
                    <code style={{
                      flex: 1,
                      fontFamily: 'Share Tech Mono, monospace',
                      fontSize: '12px',
                      color: colors.amber,
                      background: colors.void,
                      padding: '8px 12px',
                      border: `1px solid ${colors.amber}`,
                    }}>
                      {createdClient.client_secret}
                    </code>
                    <GlowButton
                      size="small"
                      variant="cyan"
                      onClick={() => copyToClipboard(createdClient.client_secret || '', 'client-secret')}
                    >
                      {copied === 'client-secret' ? '✓' : '📋'}
                    </GlowButton>
                  </div>
                </div>
              </div>

              <div style={{
                fontSize: '11px',
                color: colors.textMuted,
                marginBottom: 16,
              }}>
                Get an access token:
                <pre style={{
                  background: colors.panel,
                  padding: 12,
                  marginTop: 8,
                  border: `1px solid ${colors.textMuted}`,
                  overflow: 'auto',
                  color: colors.textSecondary,
                }}>
{`curl -X POST ${getServerBaseUrl()}/api/oauth/token \\
  -H 'Content-Type: application/json' \\
  -d '{"client_id": "${createdClient.client_id}", "client_secret": "${createdClient.client_secret}"}'`}
                </pre>
              </div>

              <GlowButton
                size="small"
                variant="cyan"
                onClick={() => setCreatedClient(null)}
              >
                DISMISS
              </GlowButton>
            </div>
          </CRTFrame>
        )}

        {/* OAuth Clients Section */}
        <CRTFrame
          title="OAUTH CLIENTS"
          subtitle="Manage MCP access credentials"
          status="online"
        >
          <div style={{ padding: 16 }}>
            {/* Create button / form */}
            {showCreateForm ? (
              <div style={{
                display: 'flex',
                alignItems: 'center',
                gap: 12,
                marginBottom: 24,
                padding: 16,
                background: colors.panel,
                border: `1px solid ${colors.cyan}`,
              }}>
                <input
                  type="text"
                  placeholder="Client name (e.g., 'My Claude Code')"
                  value={newClientName}
                  onChange={(e) => setNewClientName(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && createClient()}
                  style={{
                    flex: 1,
                    background: colors.void,
                    border: `1px solid ${colors.textMuted}`,
                    color: colors.textPrimary,
                    padding: '10px 14px',
                    fontFamily: 'Share Tech Mono, monospace',
                    fontSize: '13px',
                  }}
                />
                <GlowButton size="small" variant="green" onClick={createClient}>
                  CREATE
                </GlowButton>
                <GlowButton size="small" variant="red" onClick={() => setShowCreateForm(false)}>
                  CANCEL
                </GlowButton>
              </div>
            ) : (
              <div style={{ marginBottom: 24 }}>
                <GlowButton variant="cyan" onClick={() => setShowCreateForm(true)}>
                  + CREATE OAUTH CLIENT
                </GlowButton>
              </div>
            )}

            {/* Loading state */}
            {isLoading && (
              <div style={{
                textAlign: 'center',
                padding: 40,
                color: colors.cyan,
                fontFamily: 'VT323, monospace',
                fontSize: '16px',
              }}>
                LOADING CLIENTS...
              </div>
            )}

            {/* Clients list */}
            {!isLoading && clients.length === 0 && (
              <div style={{
                textAlign: 'center',
                padding: 40,
                color: colors.textMuted,
                fontSize: '13px',
              }}>
                No OAuth clients. Create one to enable MCP access.
              </div>
            )}

            {!isLoading && clients.length > 0 && (
              <div style={{ display: 'grid', gap: 12 }}>
                {clients.map((client) => (
                  <div
                    key={client.client_id}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'space-between',
                      padding: 16,
                      background: colors.surface,
                      border: `1px solid ${colors.panel}`,
                    }}
                  >
                    <div>
                      <div style={{
                        fontFamily: 'VT323, monospace',
                        fontSize: '16px',
                        color: colors.textPrimary,
                        marginBottom: 4,
                      }}>
                        {client.name}
                      </div>
                      <div style={{
                        fontSize: '10px',
                        color: colors.textMuted,
                        fontFamily: 'Share Tech Mono, monospace',
                      }}>
                        {client.client_id}
                      </div>
                      <div style={{
                        fontSize: '10px',
                        color: colors.textMuted,
                        marginTop: 4,
                      }}>
                        Created: {new Date(client.created_at).toLocaleDateString()}
                        {client.last_used && ` · Last used: ${new Date(client.last_used).toLocaleDateString()}`}
                      </div>
                    </div>

                    <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                      <div style={{
                        fontSize: '10px',
                        color: colors.green,
                        textTransform: 'uppercase',
                        letterSpacing: '1px',
                      }}>
                        {client.scopes.join(', ')}
                      </div>
                      <GlowButton
                        size="small"
                        variant="red"
                        onClick={() => revokeClient(client.client_id)}
                      >
                        REVOKE
                      </GlowButton>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </CRTFrame>
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
        <span>MCP ACCESS MANAGEMENT</span>
      </footer>
    </div>
  )
}
