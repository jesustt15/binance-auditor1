import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import type { PagoBinance } from '../types'
import ReportTable from '../components/ReportTable'

export default function Reports() {
  const [desde, setDesde] = useState('')
  const [hasta, setHasta] = useState('')
  const [pagos, setPagos] = useState<PagoBinance[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const buscarReportes = async () => {
    if (!desde || !hasta) return
    setLoading(true)
    setError(null)
    try {
      const result = await invoke<PagoBinance[]>('get_reports', {
        desde,
        hasta,
      })
      setPagos(result)
    } catch (err: any) {
      setError(err)
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="max-w-5xl mx-auto">
      <div className="mb-8 border-b border-slate-800 pb-4">
        <h1 className="text-2xl font-bold tracking-tight">Reportes de Auditoria</h1>
        <p className="text-slate-400 text-sm">Historial de pagos por rango de fechas</p>
      </div>

      <div className="bg-slate-800 p-6 rounded-xl border border-slate-700 mb-6">
        <div className="flex gap-4 items-end">
          <div className="flex-1">
            <label className="block text-xs font-medium text-slate-400 mb-1">Desde</label>
            <input
              type="date"
              value={desde}
              onChange={(e) => setDesde(e.target.value)}
              className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-amber-500"
            />
          </div>
          <div className="flex-1">
            <label className="block text-xs font-medium text-slate-400 mb-1">Hasta</label>
            <input
              type="date"
              value={hasta}
              onChange={(e) => setHasta(e.target.value)}
              className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-amber-500"
            />
          </div>
          <button
            onClick={buscarReportes}
            disabled={loading || !desde || !hasta}
            className="bg-blue-600 hover:bg-blue-700 disabled:bg-slate-700 disabled:cursor-not-allowed text-white font-medium px-6 py-2 rounded-lg text-sm transition-colors"
          >
            {loading ? 'Buscando...' : 'Buscar'}
          </button>
        </div>
        {error && (
          <p className="mt-3 text-sm text-rose-400">{error}</p>
        )}
      </div>

      {pagos.length > 0 || error ? (
        <div className="bg-slate-800/50 p-6 rounded-xl border border-slate-700">
          <ReportTable pagos={pagos} />
        </div>
      ) : (
        <div className="text-center text-slate-500 py-12 text-sm">
          Selecciona un rango de fechas para ver el historial de pagos.
        </div>
      )}
    </div>
  )
}
