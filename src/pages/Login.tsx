import { useState, type FormEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import type { AuthState } from '../types/auth'

interface Props {
  onLogin: (state: AuthState) => void
}

export default function Login({ onLogin }: Props) {
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()
    setError(null)
    setLoading(true)

    try {
      // Invoke login through Tauri (which proxies to server in client mode)
      const result = await invoke<{
        token: string
        refresh_token: string
        user: {
          id: string
          username: string
          role: 'admin' | 'cashier'
          company_group: string | null
          station_name: string | null
          is_active: boolean
          created_at: string
        }
      }>('client_login', { username, password })

      onLogin({
        token: result.token,
        refreshToken: result.refresh_token,
        user: result.user,
        isAuthenticated: true,
        isLoading: false,
        error: null,
      })
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err?.message || 'Error de autenticacion')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="min-h-screen bg-slate-900 flex items-center justify-center">
      <div className="bg-slate-800 p-8 rounded-xl border border-slate-700 w-full max-w-md">
        <div className="text-center mb-8">
          <h1 className="text-2xl font-bold text-slate-100">Binance Pay Auditor</h1>
          <p className="text-slate-400 text-sm mt-2">Inicia sesion para continuar</p>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-slate-300 mb-2">
              Usuario
            </label>
            <input
              type="text"
              required
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-amber-500 text-slate-200"
              placeholder="usuario"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-slate-300 mb-2">
              Contrasena
            </label>
            <input
              type="password"
              required
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-amber-500 text-slate-200"
              placeholder="••••••••"
            />
          </div>

          <button
            type="submit"
            disabled={loading}
            className="w-full bg-amber-500 hover:bg-amber-600 disabled:bg-slate-700 disabled:cursor-not-allowed text-slate-950 font-medium py-2 rounded-lg text-sm transition-colors"
          >
            {loading ? 'Autenticando...' : 'Iniciar Sesion'}
          </button>

          {error && (
            <p className="text-sm text-rose-400 text-center">{error}</p>
          )}
        </form>
      </div>
    </div>
  )
}
