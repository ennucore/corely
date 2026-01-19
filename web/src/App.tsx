import React from 'react'
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom'
import { globalStyles } from './styles'
import { AuthContext, useAuthProvider } from './hooks/useAuth'
import { LoginPage } from './pages/LoginPage'
import { DashboardPage } from './pages/DashboardPage'
import { WorkerDetailPage } from './pages/WorkerDetailPage'
import { SettingsPage } from './pages/SettingsPage'
import { CollectionConfigPage } from './pages/CollectionConfigPage'

// Inject global styles
const styleSheet = document.createElement('style')
styleSheet.textContent = globalStyles
document.head.appendChild(styleSheet)

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const auth = React.useContext(AuthContext)

  if (auth?.isLoading) {
    return (
      <div style={{
        minHeight: '100vh',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        background: '#000',
        color: '#00ffff',
        fontFamily: 'VT323, monospace',
        fontSize: '24px',
      }}>
        ◐ INITIALIZING SYSTEMS...
      </div>
    )
  }

  if (!auth?.isAuthenticated) {
    return <Navigate to="/login" replace />
  }

  return <>{children}</>
}

function App() {
  const auth = useAuthProvider()

  return (
    <AuthContext.Provider value={auth}>
      <BrowserRouter>
        <Routes>
          <Route
            path="/login"
            element={
              auth.isAuthenticated ? <Navigate to="/" replace /> : <LoginPage />
            }
          />
          <Route
            path="/"
            element={
              <ProtectedRoute>
                <DashboardPage />
              </ProtectedRoute>
            }
          />
          <Route
            path="/worker/:id"
            element={
              <ProtectedRoute>
                <WorkerDetailPage />
              </ProtectedRoute>
            }
          />
          <Route
            path="/worker/:id/collection"
            element={
              <ProtectedRoute>
                <CollectionConfigPage />
              </ProtectedRoute>
            }
          />
          <Route
            path="/settings"
            element={
              <ProtectedRoute>
                <SettingsPage />
              </ProtectedRoute>
            }
          />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </BrowserRouter>
    </AuthContext.Provider>
  )
}

export default App
