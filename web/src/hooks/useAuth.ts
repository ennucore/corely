import { useState, useEffect, createContext, useContext } from 'react'
import { api } from '../api/client'

interface AuthContextType {
  isAuthenticated: boolean
  isLoading: boolean
  user: { username: string; scopes: string[] } | null
  login: (username: string, password: string) => Promise<void>
  logout: () => void
}

export const AuthContext = createContext<AuthContextType | null>(null)

export function useAuth() {
  const context = useContext(AuthContext)
  if (!context) {
    throw new Error('useAuth must be used within AuthProvider')
  }
  return context
}

export function useAuthProvider() {
  const [isAuthenticated, setIsAuthenticated] = useState(false)
  const [isLoading, setIsLoading] = useState(true)
  const [user, setUser] = useState<{ username: string; scopes: string[] } | null>(null)

  useEffect(() => {
    const checkAuth = async () => {
      const token = api.getToken()
      if (token) {
        try {
          const userData = await api.getMe()
          setUser(userData)
          setIsAuthenticated(true)
        } catch {
          api.clearToken()
          setIsAuthenticated(false)
        }
      }
      setIsLoading(false)
    }
    checkAuth()
  }, [])

  const login = async (username: string, password: string) => {
    await api.login(username, password)
    const userData = await api.getMe()
    setUser(userData)
    setIsAuthenticated(true)
  }

  const logout = () => {
    api.clearToken()
    setUser(null)
    setIsAuthenticated(false)
  }

  return { isAuthenticated, isLoading, user, login, logout }
}
