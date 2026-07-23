import { BrowserRouter, Routes, Route, Link, useLocation, Navigate } from 'react-router-dom'
import { useState, useEffect, createContext, useContext } from 'react'
import { invoke } from '@tauri-apps/api/core'
import Dashboard from './pages/Dashboard'
import Reports from './pages/Reports'
import Settings from './pages/Settings'
import Login from './pages/Login'
import Users from './pages/Users'
import ChangePassword from './pages/ChangePassword'
import type { AuthState } from './types/auth'

// Contexto de autenticacion
const AuthContext = createContext<{
  auth: AuthState
  setAuth: (state: AuthState) => void
  logout: () => void
}>({
  auth: { token: null, refreshToken: null, user: null, isAuthenticated: false, isLoading: true, error: null },
  setAuth: () => {},
  logout: () => {},
})

export const useAuth = () => useContext(AuthContext)

function AuthGuard({ children }: { children: React.ReactNode }) {
  const { auth } = useAuth()

  if (auth.isLoading) {
    return (
      <div className="min-h-screen bg-slate-900 flex flex-col items-center justify-center gap-6">
        <svg width="64" height="64" viewBox="0 0 48 46" fill="none" xmlns="http://www.w3.org/2000/svg">
          <path d="M25.946 44.938c-.664.845-2.021.375-2.021-.698V33.937a2.26 2.26 0 0 0-2.262-2.262H10.287c-.92 0-1.456-1.04-.92-1.788l7.48-10.471c1.07-1.497 0-3.578-1.842-3.578H1.237c-.92 0-1.456-1.04-.92-1.788L10.013.474c.214-.297.556-.474.92-.474h28.894c.92 0 1.456 1.04.92 1.788l-7.48 10.471c-1.07 1.498 0 3.579 1.842 3.579h11.377c.943 0 1.473 1.088.89 1.83L25.947 44.94z" fill="#f59e0b"/>
          <path d="M25.842 44.938c-.664.844-2.021.375-2.021-.698V33.937a2.26 2.26 0 0 0-2.262-2.262H10.183c-.92 0-1.456-1.04-.92-1.788l7.48-10.471c1.07-1.498 0-3.579-1.842-3.579H1.133c-.92 0-1.456-1.04-.92-1.787L9.91.473c.214-.297.556-.474.92-.474h28.894c.92 0 1.456 1.04.92 1.788l-7.48 10.471c-1.07 1.498 0 3.578 1.842 3.578h11.377c.943 0 1.473 1.088.89 1.832L25.843 44.94z" fill="#f59e0b" opacity="0.15"/>
        </svg>
        <div className="w-7 h-7 border-[3px] border-slate-700 border-t-amber-400 rounded-full animate-spin" />
        <p className="text-amber-400 text-sm font-medium tracking-wider">Cargando...</p>
      </div>
    )
  }

  if (!auth.isAuthenticated) {
    return <Navigate to="/login" replace />
  }

  return <>{children}</>
}

function Nav() {
  const location = useLocation()
  const { auth, logout } = useAuth()

  const linkClass = (path: string) =>
    `px-3 py-2 rounded-lg text-sm font-medium transition-colors ${
      location.pathname === path
        ? 'bg-slate-800 text-amber-400'
        : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800/50'
    }`

  return (
    <nav className="bg-slate-900 border-b border-slate-800">
      <div className="max-w-5xl mx-auto px-8 flex gap-1 items-center justify-between">
        <div className="flex gap-1">
          <Link to="/" className={linkClass('/')}>
            Conciliacion
          </Link>
          <Link to="/reportes" className={linkClass('/reportes')}>
            Reportes
          </Link>
          {auth.user?.role === 'admin' && (
            <Link to="/ajustes" className={linkClass('/ajustes')}>
              Ajustes
            </Link>
          )}
          {auth.user?.role === 'admin' && (
            <Link to="/usuarios" className={linkClass('/usuarios')}>
              Usuarios
            </Link>
          )}
        </div>
        <div className="flex items-center gap-3">
          <span className="text-xs text-slate-500">
            {auth.user?.username}
            <span className={`ml-1.5 px-1.5 py-0.5 rounded text-[10px] font-medium ${
              auth.user?.role === 'admin' ? 'bg-amber-950 text-amber-400' : 'bg-slate-800 text-slate-400'
            }`}>
              {auth.user?.role}
            </span>
          </span>
          <button
            onClick={logout}
            className="text-xs text-slate-500 hover:text-rose-400 transition-colors"
          >
            Salir
          </button>
        </div>
      </div>
    </nav>
  )
}

export default function App() {
  const [auth, setAuth] = useState<AuthState>({
    token: null,
    refreshToken: null,
    user: null,
    isAuthenticated: false,
    isLoading: true,
    error: null,
  })

  // Detect operation mode on mount
  useEffect(() => {
    invoke<string>('get_app_mode')
      .then((mode) => {
        if (mode === 'standalone') {
          // Standalone mode — auto-authenticate
          setAuth({
            token: null,
            refreshToken: null,
            user: {
              id: 'local',
              username: 'admin',
              role: 'admin',
              station_name: 'Local',
              is_active: true,
              created_at: new Date().toISOString(),
            },
            isAuthenticated: true,
            isLoading: false,
            error: null,
          })
        } else {
          // Client mode — require login
          setAuth((prev) => ({ ...prev, isLoading: false }))
        }
      })
      .catch(() => {
        // If mode detection fails, default to standalone
        setAuth({
          token: null,
          refreshToken: null,
          user: {
            id: 'local',
            username: 'admin',
            role: 'admin',
            station_name: 'Local',
            is_active: true,
            created_at: new Date().toISOString(),
          },
          isAuthenticated: true,
          isLoading: false,
          error: null,
        })
      })
  }, [])

  const handleLogin = (newState: AuthState) => {
    setAuth(newState)
  }

  const logout = () => {
    setAuth({
      token: null,
      refreshToken: null,
      user: null,
      isAuthenticated: false,
      isLoading: false,
      error: null,
    })
  }

  return (
    <AuthContext.Provider value={{ auth, setAuth, logout }}>
      <BrowserRouter>
        <div className="min-h-screen bg-slate-900 text-slate-100">
          {auth.isAuthenticated && !auth.user?.must_change_password && <Nav />}
          <main className={auth.isAuthenticated && !auth.user?.must_change_password ? 'p-8' : ''}>
            <Routes>
              <Route path="/login" element={
                auth.isAuthenticated
                  ? <Navigate to="/" replace />
                  : <Login onLogin={handleLogin} />
              } />
              <Route path="/cambiar-clave" element={
                auth.isAuthenticated
                  ? auth.user?.must_change_password
                    ? <ChangePassword onPasswordChanged={() => {
                        setAuth((prev) => ({
                          ...prev,
                          user: prev.user ? { ...prev.user, must_change_password: false } : null,
                        }))
                      }} />
                    : <Navigate to="/" replace />
                  : <Navigate to="/login" replace />
              } />
              <Route path="/" element={
                auth.user?.must_change_password
                  ? <Navigate to="/cambiar-clave" replace />
                  : <AuthGuard><Dashboard /></AuthGuard>
              } />
              <Route path="/reportes" element={
                auth.user?.must_change_password
                  ? <Navigate to="/cambiar-clave" replace />
                  : <AuthGuard><Reports /></AuthGuard>
              } />
              <Route path="/ajustes" element={
                auth.user?.must_change_password
                  ? <Navigate to="/cambiar-clave" replace />
                  : <AuthGuard>
                      {auth.user?.role === 'admin' ? <Settings /> : <Navigate to="/" replace />}
                    </AuthGuard>
              } />
              <Route path="/usuarios" element={
                auth.user?.must_change_password
                  ? <Navigate to="/cambiar-clave" replace />
                  : <AuthGuard>
                      {auth.user?.role === 'admin' ? <Users /> : <Navigate to="/" replace />}
                    </AuthGuard>
              } />
            </Routes>
          </main>
        </div>
      </BrowserRouter>
    </AuthContext.Provider>
  )
}
