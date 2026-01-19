import { useState } from 'react'
import { colors } from '../../styles'
import { CollectionConfig } from '../../api/client'
import { GlowButton } from '../GlowButton'

interface ConfigEditorProps {
  config: CollectionConfig
  onChange: (config: CollectionConfig) => void
  onSave: () => void
  isSaving?: boolean
}

const inputStyle = {
  background: colors.deep,
  border: `1px solid ${colors.cyan}40`,
  color: colors.cyan,
  fontFamily: 'VT323, monospace',
  fontSize: '16px',
  padding: '8px 12px',
  borderRadius: '4px',
  outline: 'none',
  width: '100%',
}

const checkboxStyle = {
  width: '20px',
  height: '20px',
  cursor: 'pointer',
}

const labelStyle = {
  fontFamily: 'VT323, monospace',
  fontSize: '18px',
  color: colors.text,
  display: 'flex',
  alignItems: 'center',
  gap: '10px',
}

const sectionStyle = {
  background: `${colors.surface}80`,
  border: `1px solid ${colors.cyan}30`,
  borderRadius: '8px',
  padding: '20px',
  marginBottom: '20px',
}

const sectionTitleStyle = {
  fontFamily: 'VT323, monospace',
  fontSize: '22px',
  color: colors.cyan,
  marginBottom: '15px',
  textShadow: `0 0 10px ${colors.cyan}`,
}

export function ConfigEditor({ config, onChange, onSave, isSaving }: ConfigEditorProps) {
  const updateScreen = (updates: Partial<typeof config.screen>) => {
    onChange({ ...config, screen: { ...config.screen, ...updates } })
  }

  const updateCamera = (updates: Partial<typeof config.camera>) => {
    onChange({ ...config, camera: { ...config.camera, ...updates } })
  }

  const updateAudioInput = (updates: Partial<typeof config.audio_input>) => {
    onChange({ ...config, audio_input: { ...config.audio_input, ...updates } })
  }

  const updateAudioOutput = (updates: Partial<typeof config.audio_output>) => {
    onChange({ ...config, audio_output: { ...config.audio_output, ...updates } })
  }

  const updateInputLogging = (updates: Partial<typeof config.input_logging>) => {
    onChange({ ...config, input_logging: { ...config.input_logging, ...updates } })
  }

  const updateDirectorySync = (updates: Partial<typeof config.directory_sync>) => {
    onChange({ ...config, directory_sync: { ...config.directory_sync, ...updates } })
  }

  return (
    <div>
      {/* Screen Capture */}
      <div style={sectionStyle}>
        <div style={sectionTitleStyle}>SCREEN CAPTURE</div>
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '15px' }}>
          <label style={labelStyle}>
            <input
              type="checkbox"
              checked={config.screen.enabled}
              onChange={(e) => updateScreen({ enabled: e.target.checked })}
              style={checkboxStyle}
            />
            Enable Screen Capture
          </label>
          <div style={labelStyle}>
            FPS:
            <input
              type="number"
              min="1"
              max="30"
              value={config.screen.fps}
              onChange={(e) => updateScreen({ fps: parseInt(e.target.value) || 1 })}
              style={{ ...inputStyle, width: '80px' }}
              disabled={!config.screen.enabled}
            />
          </div>
          <div style={labelStyle}>
            Resolution:
            <select
              value={config.screen.resolution}
              onChange={(e) => updateScreen({ resolution: parseInt(e.target.value) })}
              style={{ ...inputStyle, width: '120px' }}
              disabled={!config.screen.enabled}
            >
              <option value="480">480p</option>
              <option value="720">720p</option>
              <option value="1080">1080p</option>
              <option value="1440">1440p</option>
            </select>
          </div>
          <div style={labelStyle}>
            Quality:
            <input
              type="range"
              min="1"
              max="100"
              value={config.screen.quality}
              onChange={(e) => updateScreen({ quality: parseInt(e.target.value) })}
              style={{ width: '100px' }}
              disabled={!config.screen.enabled}
            />
            <span style={{ color: colors.cyan }}>{config.screen.quality}%</span>
          </div>
        </div>
      </div>

      {/* Camera Capture */}
      <div style={sectionStyle}>
        <div style={sectionTitleStyle}>CAMERA CAPTURE</div>
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '15px' }}>
          <label style={labelStyle}>
            <input
              type="checkbox"
              checked={config.camera.enabled}
              onChange={(e) => updateCamera({ enabled: e.target.checked })}
              style={checkboxStyle}
            />
            Enable Camera Capture
          </label>
          <div style={labelStyle}>
            FPS:
            <input
              type="number"
              min="1"
              max="30"
              value={config.camera.fps}
              onChange={(e) => updateCamera({ fps: parseInt(e.target.value) || 5 })}
              style={{ ...inputStyle, width: '80px' }}
              disabled={!config.camera.enabled}
            />
          </div>
          <label style={labelStyle}>
            <input
              type="checkbox"
              checked={config.camera.implies_mic}
              onChange={(e) => updateCamera({ implies_mic: e.target.checked })}
              style={checkboxStyle}
              disabled={!config.camera.enabled}
            />
            Auto-enable Microphone
          </label>
        </div>
      </div>

      {/* Audio */}
      <div style={sectionStyle}>
        <div style={sectionTitleStyle}>AUDIO CAPTURE</div>
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '15px' }}>
          <label style={labelStyle}>
            <input
              type="checkbox"
              checked={config.audio_input.enabled}
              onChange={(e) => updateAudioInput({ enabled: e.target.checked })}
              style={checkboxStyle}
            />
            Microphone
          </label>
          <label style={labelStyle}>
            <input
              type="checkbox"
              checked={config.audio_output.enabled}
              onChange={(e) => updateAudioOutput({ enabled: e.target.checked })}
              style={checkboxStyle}
            />
            System Audio (Loopback)
          </label>
        </div>
      </div>

      {/* Input Logging */}
      <div style={sectionStyle}>
        <div style={sectionTitleStyle}>INPUT LOGGING</div>
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: '15px' }}>
          <label style={labelStyle}>
            <input
              type="checkbox"
              checked={config.input_logging.enabled}
              onChange={(e) => updateInputLogging({ enabled: e.target.checked })}
              style={checkboxStyle}
            />
            Enable Input Logging
          </label>
          <label style={{ ...labelStyle, opacity: config.input_logging.enabled ? 1 : 0.5 }}>
            <input
              type="checkbox"
              checked={config.input_logging.log_keystrokes}
              onChange={(e) => updateInputLogging({ log_keystrokes: e.target.checked })}
              style={checkboxStyle}
              disabled={!config.input_logging.enabled}
            />
            Keystrokes
          </label>
          <label style={{ ...labelStyle, opacity: config.input_logging.enabled ? 1 : 0.5 }}>
            <input
              type="checkbox"
              checked={config.input_logging.log_mouse_clicks}
              onChange={(e) => updateInputLogging({ log_mouse_clicks: e.target.checked })}
              style={checkboxStyle}
              disabled={!config.input_logging.enabled}
            />
            Mouse Clicks
          </label>
        </div>
      </div>

      {/* Directory Sync */}
      <div style={sectionStyle}>
        <div style={sectionTitleStyle}>DIRECTORY SYNC</div>
        <div style={{ marginBottom: '15px' }}>
          <label style={{ ...labelStyle, marginBottom: '10px' }}>
            Paths to sync (one per line):
          </label>
          <textarea
            value={config.directory_sync.paths.join('\n')}
            onChange={(e) => updateDirectorySync({
              paths: e.target.value.split('\n').filter(p => p.trim())
            })}
            style={{ ...inputStyle, height: '80px', resize: 'vertical' }}
            placeholder="/home/user/documents&#10;/home/user/projects"
          />
        </div>
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '15px' }}>
          <div style={labelStyle}>
            Sync Interval:
            <input
              type="number"
              min="60"
              max="86400"
              value={config.directory_sync.sync_interval_secs}
              onChange={(e) => updateDirectorySync({ sync_interval_secs: parseInt(e.target.value) || 300 })}
              style={{ ...inputStyle, width: '100px' }}
            />
            <span style={{ color: colors.text }}>seconds</span>
          </div>
          <label style={labelStyle}>
            <input
              type="checkbox"
              checked={config.directory_sync.watch_changes}
              onChange={(e) => updateDirectorySync({ watch_changes: e.target.checked })}
              style={checkboxStyle}
            />
            Watch for Changes
          </label>
        </div>
      </div>

      {/* General Settings */}
      <div style={sectionStyle}>
        <div style={sectionTitleStyle}>GENERAL</div>
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '15px' }}>
          <div style={labelStyle}>
            Chunk Duration:
            <input
              type="number"
              min="10"
              max="300"
              value={config.chunk_duration_secs}
              onChange={(e) => onChange({ ...config, chunk_duration_secs: parseInt(e.target.value) || 60 })}
              style={{ ...inputStyle, width: '100px' }}
            />
            <span style={{ color: colors.text }}>seconds</span>
          </div>
        </div>
      </div>

      {/* Save Button */}
      <div style={{ display: 'flex', justifyContent: 'flex-end', marginTop: '20px' }}>
        <GlowButton onClick={onSave} disabled={isSaving}>
          {isSaving ? 'SAVING...' : 'SAVE CONFIGURATION'}
        </GlowButton>
      </div>
    </div>
  )
}
