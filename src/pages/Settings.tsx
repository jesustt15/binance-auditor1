import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useAuth } from '../App'
import { api } from '../lib/api'
import type { AppSettings } from '../types'

export default function Settings() {
  const { auth } = useAuth()
  const [imapUser, setImapUser] = useState('')
  const [imapPassword, setImapPassword] = useState('')
  const [saved, setSaved] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [appMode, setAppMode] = useState<string>('standalone')

  const isAdmin = auth.user?.role === 'admin'

  useEffect(() => {
    // Detect app mode first
    invoke<string>('get_app_mode')
      .then((mode) => {
        setAppMode(mode)
      })
      .catch(() => {})

    invoke<AppSettings>('get_settings')
      .then((settings) => {
        setImapUser(settings.imap_user)
        setImapPassword(settings.imap_password)
      })
      .catch(() => {})

    // In client mode, fetch server IMAP config if admin
    if (auth.user?.role === 'admin') {
      api.getImapConfig()
        .then((cfg) => {
          if (cfg.email) setImapUser(cfg.email)
        })
        .catch(() => {})
    }

    setLoading(false)
  }, [])

  const handleSave = async () => {
    setError(null)
    setSaved(false)
    try {
      // In client mode, use the server API command
      if (appMode === 'client') {
        await invoke('client_save_imap_config', {
          email: imapUser,
          password: imapPassword,
          host: 'imap.gmail.com',
          port: 993,
        })
      } else {
        await invoke('save_settings', {
          imapUser,
          imapPassword,
        })
      }
      setSaved(true)
      setTimeout(() => setSaved(false), 3000)
    } catch (err: any) {
      setError(err)
    }
  }

  if (loading) {
    return (
      <div className="max-w-2xl mx-auto text-center text-slate-500 py-12">
        Cargando configuracion...
      </div>
    )
  }

  // Client mode: show IMAP config (editable for admin)
  if (appMode === 'client') {
    return (
      <div className="max-w-2xl mx-auto">
        <div className="mb-8 border-b border-slate-800 pb-4">
          <h1 className="text-2xl font-bold tracking-tight">Configuracion</h1>
          <p className="text-slate-400 text-sm">Modo Cliente — conectado al servidor central</p>
        </div>

        <div className="bg-slate-800 p-6 rounded-xl border border-slate-700 space-y-6">
          {isAdmin ? (
            <>
              <div>
                <label className="block text-sm font-medium text-slate-300 mb-2">
                  Correo de Gmail (IMAP)
                </label>
                <input
                  type="email"
                  value={imapUser}
                  onChange={(e) => setImapUser(e.target.value)}
                  className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-amber-500"
                  placeholder="admin@correo.com"
                />
              </div>

              <div>
                <label className="block text-sm font-medium text-slate-300 mb-2">
                  Contrasena de aplicacion (16 caracteres)
                </label>
                <input
                  type="password"
                  value={imapPassword}
                  onChange={(e) => setImapPassword(e.target.value)}
                  className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-amber-500"
                  placeholder="abcd efgh ijkl mnop"
                />
                <p className="mt-2 text-xs text-slate-500">
                  Genera una "Contrasena de aplicacion" en{' '}
                  <a href="https://myaccount.google.com/apppasswords" target="_blank" className="text-amber-400 hover:underline ml-1">
                    Google Account &rarr; App Passwords
                  </a>
                </p>
              </div>

              <button
                onClick={handleSave}
                className="bg-amber-500 hover:bg-amber-600 text-slate-950 font-medium px-6 py-2 rounded-lg text-sm transition-colors"
              >
                Guardar en Servidor
              </button>

              {saved && (
                <p className="text-sm text-emerald-400">Credenciales IMAP guardadas en el servidor.</p>
              )}
            </>
          ) : (
            <div className="p-4 bg-blue-950/50 border border-blue-500/30 rounded-lg">
              <p className="text-blue-300 text-sm font-medium">Modo Cliente Activo</p>
              <p className="text-blue-400/70 text-xs mt-1">
                Las credenciales IMAP se configuran centralmente en el servidor por el administrador.
              </p>
            </div>
          )}

          {error && (
            <p className="text-sm text-rose-400">Error al guardar: {error}</p>
          )}
        </div>
      </div>
    )
  }

  // Standalone mode — original IMAP config UI
  return (
    <div className="max-w-2xl mx-auto">
      <div className="mb-8 border-b border-slate-800 pb-4">
        <h1 className="text-2xl font-bold tracking-tight">Configuracion</h1>
        <p className="text-slate-400 text-sm">Credenciales de Gmail para sincronizacion de correos</p>
      </div>

      <div className="bg-slate-800 p-6 rounded-xl border border-slate-700 space-y-6">
        <div>
          <label className="block text-sm font-medium text-slate-300 mb-2">
            Correo de Gmail (IMAP)
          </label>
          <input
            type="email"
            value={imapUser}
            onChange={(e) => setImapUser(e.target.value)}
            className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-amber-500"
            placeholder="tu-correo@gmail.com"
          />
        </div>

        <div>
          <label className="block text-sm font-medium text-slate-300 mb-2">
            Contrasena de aplicacion (16 caracteres)
          </label>
          <input
            type="password"
            value={imapPassword}
            onChange={(e) => setImapPassword(e.target.value)}
            className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-amber-500"
            placeholder="abcd efgh ijkl mnop"
          />
          <p className="mt-2 text-xs text-slate-500">
            No uses tu contrasena normal de Gmail. Genera una "Contrasena de aplicacion" en
            <a href="https://myaccount.google.com/apppasswords" target="_blank" className="text-amber-400 hover:underline ml-1">
              Google Account &rarr; App Passwords
            </a>
          </p>
        </div>

        <button
          onClick={handleSave}
          className="bg-amber-500 hover:bg-amber-600 text-slate-950 font-medium px-6 py-2 rounded-lg text-sm transition-colors"
        >
          Guardar Configuracion
        </button>

        {saved && (
          <p className="text-sm text-emerald-400">
            Configuracion guardada correctamente.
          </p>
        )}

        {error && (
          <p className="text-sm text-rose-400">
            Error al guardar: {error}
          </p>
        )}
      </div>
    </div>
  )
}
