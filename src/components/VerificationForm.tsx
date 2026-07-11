import { useState, type FormEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import type { VerifyResult } from '../types'

export default function VerificationForm() {
  const [usuario, setUsuario] = useState('')
  const [monto, setMonto] = useState('')
  const [fecha, setFecha] = useState('')
  const [resultado, setResultado] = useState<VerifyResult | null>(null)
  const [loadingVerificar, setLoadingVerificar] = useState(false)

  const manejarVerificacion = async (e: FormEvent) => {
    e.preventDefault()
    setLoadingVerificar(true)
    setResultado(null)

    try {
      const result = await invoke<VerifyResult>('verify_payment', {
        usuarioEmpresa: usuario,
        montoEmpresa: parseFloat(monto),
        fechaEmpresa: fecha,
      })
      setResultado(result)
    } catch (err: any) {
      setResultado({ verificado: false, mensaje: `Error: ${err}` })
    } finally {
      setLoadingVerificar(false)
    }
  }

  return (
    <div className="bg-slate-800 p-6 rounded-xl border border-slate-700">
      <h2 className="text-lg font-semibold mb-4 text-amber-400">Datos Reportados por la Empresa</h2>
      <form onSubmit={manejarVerificacion} className="space-y-4">
        <div>
          <label className="block text-xs font-medium text-slate-400 mb-1">Usuario de Binance</label>
          <input
            type="text"
            required
            value={usuario}
            onChange={(e) => setUsuario(e.target.value)}
            className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-amber-500"
            placeholder="Ej. JulioPerez"
          />
        </div>
        <div>
          <label className="block text-xs font-medium text-slate-400 mb-1">Monto Exacto</label>
          <input
            type="number"
            step="0.01"
            required
            value={monto}
            onChange={(e) => setMonto(e.target.value)}
            className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-amber-500"
            placeholder="$0.00"
          />
        </div>
        <div>
          <label className="block text-xs font-medium text-slate-400 mb-1">Fecha del Reporte</label>
          <input
            type="date"
            required
            value={fecha}
            onChange={(e) => setFecha(e.target.value)}
            className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-amber-500"
          />
        </div>
        <button
          type="submit"
          disabled={loadingVerificar}
          className="w-full bg-blue-600 hover:bg-blue-700 disabled:bg-slate-700 disabled:cursor-not-allowed text-white font-medium py-2 rounded-lg text-sm transition-colors"
        >
          {loadingVerificar ? 'Buscando Coincidencias...' : 'Cruzar y Verificar Pago'}
        </button>
      </form>

      {resultado && (
        <div className={`mt-4 p-4 rounded-lg text-sm font-medium border ${
          resultado.verificado
            ? 'bg-emerald-950/50 border-emerald-500/50 text-emerald-300'
            : 'bg-rose-950/50 border-rose-500/50 text-rose-300'
        }`}>
          {resultado.mensaje}
        </div>
      )}

      {resultado?.verificado && (
        <div className="mt-4 p-4 bg-slate-900/50 rounded-lg border border-slate-700 text-xs text-slate-400">
          El estado del registro ha sido marcado como{' '}
          <span className="text-emerald-400 font-bold">VERIFICADO</span>. Este pago ya no podra ser usado para otra validacion.
        </div>
      )}
    </div>
  )
}
