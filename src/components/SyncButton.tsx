import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { SyncResult, SyncProgress } from '../types'

interface Props {
  onSyncComplete: () => void
}

export default function SyncButton({ onSyncComplete }: Props) {
  const [sincronizando, setSincronizando] = useState(false)
  const [lastSync, setLastSync] = useState<SyncResult | null>(null)
  const [progreso, setProgreso] = useState<SyncProgress | null>(null)

  useEffect(() => {
    const unlisten = listen<SyncProgress>('sync-progress', (event) => {
      setProgreso(event.payload)
    })
    return () => { unlisten.then(fn => fn()) }
  }, [])

  const sincronizarCorreos = async () => {
    setSincronizando(true)
    setLastSync(null)
    setProgreso(null)
    try {
      const result = await invoke<SyncResult>('sync_emails')
      setLastSync(result)
      if (result.success) {
        onSyncComplete()
      }
    } catch (err: any) {
      setLastSync({ success: false, mensajes_nuevos: 0, total_procesados: 0, error: err })
    } finally {
      setSincronizando(false)
      setProgreso(null)
    }
  }

  const progresoPct = progreso && progreso.total > 0
    ? Math.round((progreso.actual / progreso.total) * 100)
    : 0

  return (
    <div className="flex flex-col gap-2">
      <button
        onClick={sincronizarCorreos}
        disabled={sincronizando}
        className="bg-amber-500 hover:bg-amber-600 disabled:bg-slate-700 disabled:cursor-not-allowed text-slate-950 px-4 py-2 rounded-lg font-medium transition-all text-sm flex items-center gap-2"
      >
        {sincronizando ? (
          <>
            <svg className="animate-spin h-4 w-4" viewBox="0 0 24 24">
              <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" fill="none" />
              <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
            </svg>
            {progreso && progreso.total > 0
              ? `Procesando ${progreso.actual}/${progreso.total} (${progresoPct}%)`
              : 'Conectando a Gmail...'}
          </>
        ) : (
          <>Sincronizar Correos</>
        )}
      </button>
      {sincronizando && progreso && progreso.total > 0 && (
        <div className="w-full bg-slate-700 rounded-full h-1.5">
          <div
            className="bg-amber-500 h-1.5 rounded-full transition-all duration-300"
            style={{ width: `${progresoPct}%` }}
          />
        </div>
      )}
      {lastSync && (
        <p className={`text-xs ${lastSync.success ? 'text-emerald-400' : 'text-rose-400'}`}>
          {lastSync.success
            ? `${lastSync.mensajes_nuevos} nuevo(s) pago(s) de ${lastSync.total_procesados} correo(s) procesado(s)`
            : `Error: ${lastSync.error}`}
        </p>
      )}
    </div>
  )
}
