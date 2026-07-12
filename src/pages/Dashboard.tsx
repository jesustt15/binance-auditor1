import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import SyncButton from '../components/SyncButton'
import VerificationForm from '../components/VerificationForm'
import type { PagoBinance, ImportResult } from '../types'
import { formatFecha } from '../lib/format'

export default function Dashboard() {
  const [pagosBD, setPagosBD] = useState<PagoBinance[] | null>(null)
  const [loadingBD, setLoadingBD] = useState(false)
  const [desdeBD, setDesdeBD] = useState('')
  const [hastaBD, setHastaBD] = useState('')

  // CSV import state
  const [csvImportando, setCsvImportando] = useState(false)
  const [csvResultado, setCsvResultado] = useState<ImportResult | null>(null)
  const [csvError, setCsvError] = useState<string | null>(null)

  const copiarPagos = () => {
    if (!pagosBD || pagosBD.length === 0) return
    // Extraer solo la fecha YYYY-MM-DD del RFC3339
    const toISODate = (rfc: string) => rfc.slice(0, 10)
    const csv = [
      'usuario,monto,fecha',
      ...pagosBD.map(p => `${p.usuario_remitente},${p.monto},${toISODate(p.fecha_correo)}`),
    ].join('\n')
    navigator.clipboard.writeText(csv)
  }

  const listarPagos = async () => {
    setLoadingBD(true)
    try {
      const args: Record<string, unknown> = {}
      if (desdeBD && hastaBD) {
        args.desde = desdeBD
        args.hasta = hastaBD
      }
      const pagos = await invoke<PagoBinance[]>('debug_listar_pagos', args)
      setPagosBD(pagos)
    } catch (err) {
      console.error('Error listando pagos:', err)
    } finally {
      setLoadingBD(false)
    }
  }

  const manejarArchivoCSV = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return

    setCsvImportando(true)
    setCsvResultado(null)
    setCsvError(null)

    try {
      const contenido = await file.text()
      const result = await invoke<ImportResult>('import_csv', { contenido })
      setCsvResultado(result)
    } catch (err: any) {
      setCsvError(`Error al procesar CSV: ${err}`)
    } finally {
      setCsvImportando(false)
      // Reset input so the same file can be re-selected
      e.target.value = ''
    }
  }

  return (
    <div className="max-w-4xl mx-auto">
      <div className="flex justify-between items-center mb-8 border-b border-slate-800 pb-4">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Binance Pay Auditor v1.0</h1>
          <p className="text-slate-400 text-sm">Conciliacion de pagos</p>
        </div>
        <SyncButton onSyncComplete={() => {}} />
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-8">
        <VerificationForm />

        {/* CSV Import masivo */}
        <div className="bg-slate-800 p-6 rounded-xl border border-slate-700">
          <h2 className="text-lg font-semibold mb-4 text-slate-300">Carga Masiva de Reportes (CSV)</h2>
          <p className="text-xs text-slate-500 mb-4">
            Subi un archivo CSV con columnas: <span className="text-slate-400 font-mono">usuario, monto, fecha</span>.
            Cada fila se verificara automaticamente contra los correos sincronizados.
          </p>

          <label className="block w-full cursor-pointer bg-slate-900 border-2 border-dashed border-slate-700 hover:border-amber-600/50 rounded-lg p-6 text-center transition-colors">
            <input
              type="file"
              accept=".csv"
              onChange={manejarArchivoCSV}
              disabled={csvImportando}
              className="hidden"
            />
            {csvImportando ? (
              <p className="text-amber-400 text-sm font-medium">Procesando archivo...</p>
            ) : (
              <>
                <p className="text-slate-400 text-sm font-medium">Click para seleccionar archivo CSV</p>
                <p className="text-slate-600 text-xs mt-1">o arrastra el archivo aqui</p>
              </>
            )}
          </label>

          {csvError && (
            <p className="mt-3 text-sm text-rose-400">{csvError}</p>
          )}

          {csvResultado && (
            <div className="mt-4">
              {/* Summary cards */}
              <div className="grid grid-cols-4 gap-2 mb-4">
                <div className="bg-slate-900 rounded-lg p-3 text-center">
                  <p className="text-xs text-slate-500">Total</p>
                  <p className="text-lg font-bold text-slate-300">{csvResultado.total_filas}</p>
                </div>
                <div className="bg-emerald-950/50 rounded-lg p-3 text-center">
                  <p className="text-xs text-emerald-500">Verificados</p>
                  <p className="text-lg font-bold text-emerald-400">{csvResultado.verificados}</p>
                </div>
                <div className="bg-amber-950/50 rounded-lg p-3 text-center">
                  <p className="text-xs text-amber-500">No Encontrados</p>
                  <p className="text-lg font-bold text-amber-400">{csvResultado.no_encontrados}</p>
                </div>
                <div className="bg-rose-950/50 rounded-lg p-3 text-center">
                  <p className="text-xs text-rose-500">Errores</p>
                  <p className="text-lg font-bold text-rose-400">{csvResultado.errores}</p>
                </div>
              </div>

              {/* Detail table */}
              {csvResultado.detalle.length > 0 && (
                <div className="max-h-64 overflow-y-auto">
                  <table className="w-full text-xs text-left">
                    <thead className="text-slate-500 border-b border-slate-700 sticky top-0 bg-slate-800">
                      <tr>
                        <th className="pb-1 pr-2">#</th>
                        <th className="pb-1 pr-2">Usuario</th>
                        <th className="pb-1 pr-2">Monto</th>
                        <th className="pb-1 pr-2">Fecha</th>
                        <th className="pb-1 pr-2">Resultado</th>
                      </tr>
                    </thead>
                    <tbody>
                      {csvResultado.detalle.map((row) => (
                        <tr key={row.fila} className="border-b border-slate-800/50">
                          <td className="py-1 pr-2 text-slate-600">{row.fila}</td>
                          <td className="py-1 pr-2 text-slate-400">{row.usuario}</td>
                          <td className="py-1 pr-2 text-slate-400">{row.monto}</td>
                          <td className="py-1 pr-2 text-slate-400">{formatFecha(row.fecha)}</td>
                          <td className="py-1 pr-2">
                            <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${
                              row.resultado === 'verificado' ? 'bg-emerald-950 text-emerald-400' :
                              row.resultado === 'no_encontrado' ? 'bg-amber-950 text-amber-400' :
                              'bg-rose-950 text-rose-400'
                            }`}>
                              {row.resultado}
                            </span>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      {/* Pagos en Base de Datos */}
      <div className="mt-8 bg-slate-800/50 p-6 rounded-xl border border-slate-700">
        <div className="flex justify-between items-center mb-4 flex-wrap gap-3">
          <div>
            <h2 className="text-lg font-semibold text-slate-300">Pagos en Base de Datos</h2>
            <p className="text-xs text-slate-500">Registros sincronizados desde correos de Binance</p>
          </div>
          <div className="flex gap-2 items-end">
            <div>
              <label className="block text-[10px] font-medium text-slate-500 mb-0.5">Desde</label>
              <input
                type="date"
                value={desdeBD}
                onChange={(e) => setDesdeBD(e.target.value)}
                className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-xs w-32 focus:outline-none focus:border-amber-500"
              />
            </div>
            <div>
              <label className="block text-[10px] font-medium text-slate-500 mb-0.5">Hasta</label>
              <input
                type="date"
                value={hastaBD}
                onChange={(e) => setHastaBD(e.target.value)}
                className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-xs w-32 focus:outline-none focus:border-amber-500"
              />
            </div>
            <button
              onClick={listarPagos}
              disabled={loadingBD}
              className="px-4 py-1.5 bg-amber-600 hover:bg-amber-700 disabled:bg-slate-700 text-white rounded text-xs font-medium transition-colors"
            >
              {loadingBD ? 'Cargando...' : (desdeBD && hastaBD ? 'Filtrar' : 'Listar Todos')}
            </button>
            {pagosBD && pagosBD.length > 0 && (
              <button
                onClick={copiarPagos}
                className="px-4 py-1.5 bg-slate-700 hover:bg-slate-600 text-slate-300 rounded text-xs font-medium transition-colors"
                title="Copia usuario,monto,fecha en formato CSV para carga masiva"
              >
                Copiar
              </button>
            )}
          </div>
        </div>

        {pagosBD !== null && (
          pagosBD.length === 0 ? (
            <p className="text-slate-500 text-sm">No hay pagos en la base de datos. Sincroniza primero.</p>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-xs text-left">
                <thead className="text-slate-400 border-b border-slate-700">
                  <tr>
                    <th className="pb-2 pr-4">ID</th>
                    <th className="pb-2 pr-4">Usuario</th>
                    <th className="pb-2 pr-4">Monto</th>
                    <th className="pb-2 pr-4">Moneda</th>
                    <th className="pb-2 pr-4">Fecha Correo</th>
                    <th className="pb-2 pr-4">Estado</th>
                  </tr>
                </thead>
                <tbody>
                  {pagosBD.map((p) => (
                    <tr key={p.id} className="border-b border-slate-800 hover:bg-slate-800/50">
                      <td className="py-2 pr-4 text-slate-500">{p.id}</td>
                      <td className="py-2 pr-4 font-mono text-slate-300">{p.usuario_remitente}</td>
                      <td className="py-2 pr-4 text-slate-300">{p.monto}</td>
                      <td className="py-2 pr-4 text-slate-400">{p.moneda}</td>
                      <td className="py-2 pr-4 text-slate-400">{formatFecha(p.fecha_correo)}</td>
                      <td className="py-2 pr-4">
                        <span className={`px-2 py-0.5 rounded-full text-xs font-medium ${
                          p.estado === 'verificado' ? 'bg-emerald-950 text-emerald-400' : 'bg-amber-950 text-amber-400'
                        }`}>
                          {p.estado}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <p className="mt-4 text-xs text-slate-500">
                Total: {pagosBD.length} pagos | Disponibles: {pagosBD.filter(p => p.estado === 'disponible').length} | Verificados: {pagosBD.filter(p => p.estado === 'verificado').length}
              </p>
            </div>
          )
        )}
      </div>
    </div>
  )
}
