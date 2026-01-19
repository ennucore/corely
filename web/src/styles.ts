// Retro-futuristic color palette
export const colors = {
  // Primary neons
  cyan: '#00ffff',
  magenta: '#ff00ff',
  green: '#00ff41',
  amber: '#ffb000',
  red: '#ff0040',

  // Background layers
  void: '#000000',
  deep: '#0a0a0f',
  surface: '#0d1117',
  panel: '#161b22',

  // Text
  textPrimary: '#c9d1d9',
  textSecondary: '#8b949e',
  textMuted: '#484f58',

  // Glow variants
  cyanGlow: '0 0 10px #00ffff, 0 0 20px #00ffff, 0 0 40px #00ffff',
  magentaGlow: '0 0 10px #ff00ff, 0 0 20px #ff00ff',
  greenGlow: '0 0 10px #00ff41, 0 0 20px #00ff41',
  redGlow: '0 0 10px #ff0040, 0 0 20px #ff0040',
}

// Global styles as a CSS string
export const globalStyles = `
  @keyframes scanline {
    0% { transform: translateY(-100%); }
    100% { transform: translateY(100vh); }
  }

  @keyframes flicker {
    0%, 100% { opacity: 1; }
    92% { opacity: 1; }
    93% { opacity: 0.8; }
    94% { opacity: 1; }
    97% { opacity: 0.9; }
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }

  @keyframes blink {
    0%, 50% { opacity: 1; }
    51%, 100% { opacity: 0; }
  }

  @keyframes typing {
    from { width: 0; }
    to { width: 100%; }
  }

  @keyframes glitch {
    0% { transform: translate(0); }
    20% { transform: translate(-2px, 2px); }
    40% { transform: translate(-2px, -2px); }
    60% { transform: translate(2px, 2px); }
    80% { transform: translate(2px, -2px); }
    100% { transform: translate(0); }
  }

  @keyframes bootSequence {
    0% { opacity: 0; transform: translateY(10px); }
    100% { opacity: 1; transform: translateY(0); }
  }

  * {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
  }

  html, body, #root {
    height: 100%;
    width: 100%;
  }

  body {
    font-family: 'Share Tech Mono', 'Courier New', monospace;
    background: ${colors.void};
    color: ${colors.textPrimary};
    line-height: 1.6;
  }

  /* CRT effect overlay */
  #root::before {
    content: '';
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: repeating-linear-gradient(
      0deg,
      rgba(0, 0, 0, 0.15),
      rgba(0, 0, 0, 0.15) 1px,
      transparent 1px,
      transparent 2px
    );
    pointer-events: none;
    z-index: 10000;
  }

  /* Scanline animation */
  #root::after {
    content: '';
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 10px;
    background: linear-gradient(
      to bottom,
      transparent,
      rgba(0, 255, 255, 0.03),
      transparent
    );
    animation: scanline 8s linear infinite;
    pointer-events: none;
    z-index: 10001;
  }

  ::-webkit-scrollbar {
    width: 8px;
    height: 8px;
  }

  ::-webkit-scrollbar-track {
    background: ${colors.deep};
    border: 1px solid ${colors.panel};
  }

  ::-webkit-scrollbar-thumb {
    background: ${colors.cyan};
    border: 1px solid ${colors.cyan};
  }

  ::-webkit-scrollbar-thumb:hover {
    background: ${colors.magenta};
    border-color: ${colors.magenta};
  }

  ::selection {
    background: ${colors.cyan};
    color: ${colors.void};
  }
`
