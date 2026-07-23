import { useState, useMemo } from 'react'
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
  const [montoExactoBD, setMontoExactoBD] = useState('')
  const [montoMinBD, setMontoMinBD] = useState('')
  const [montoMaxBD, setMontoMaxBD] = useState('')
  const [verificandoId, setVerificandoId] = useState<number | null>(null)
  const [paginaBD, setPaginaBD] = useState(1)
  const [porPaginaBD, setPorPaginaBD] = useState(20)

  // CSV/Excel import state
  const [importando, setImportando] = useState(false)
  const [importResultado, setImportResultado] = useState<ImportResult | null>(null)
  const [importError, setImportError] = useState<string | null>(null)

  // Historical sync state
  const [histFecha, setHistFecha] = useState('')
  const [histSyncing, setHistSyncing] = useState(false)
  const [histResultado, setHistResultado] = useState<string | null>(null)
  const [histError, setHistError] = useState<string | null>(null)

  const copiarPagos = () => {
    if (!pagosBD || pagosBD.length === 0) return
    const toISODate = (rfc: string) => rfc.slice(0, 10)
    const csv = [
      'usuario,monto,fecha',
      ...pagosBD.map(p => `${p.usuario_remitente ?? '—'},${p.monto},${toISODate(p.fecha_correo)}`),
    ].join('\n')
    navigator.clipboard.writeText(csv)
  }

  const limpiarFiltros = () => {
    setDesdeBD('')
    setHastaBD('')
    setMontoExactoBD('')
    setMontoMinBD('')
    setMontoMaxBD('')
  }

  const listarPagos = async () => {
    setLoadingBD(true)
    setPaginaBD(1)
    try {
      const args: Record<string, unknown> = {}
      if (desdeBD && hastaBD) {
        args.desde = desdeBD
        args.hasta = hastaBD
      }
      if (montoExactoBD) args.montoExacto = parseFloat(montoExactoBD)
      if (montoMinBD) args.montoMin = parseFloat(montoMinBD)
      if (montoMaxBD) args.montoMax = parseFloat(montoMaxBD)
      const pagos = await invoke<PagoBinance[]>('debug_listar_pagos', args)
      setPagosBD(pagos)
    } catch (err) {
      console.error('Error listando pagos:', err)
    } finally {
      setLoadingBD(false)
    }
  }

  const quickVerify = async (id: number) => {
    setVerificandoId(id)
    try {
      await invoke('quick_verify_pago', { id })
      // Refresh the list to show updated estado
      await listarPagos()
    } catch (err) {
      console.error('Error verificando pago:', err)
    } finally {
      setVerificandoId(null)
    }
  }

  const totalPaginasBD = useMemo(() => pagosBD ? Math.ceil(pagosBD.length / porPaginaBD) : 0, [pagosBD, porPaginaBD])

  const pagosPaginados = useMemo(() => {
    if (!pagosBD) return []
    const inicio = (paginaBD - 1) * porPaginaBD
    return pagosBD.slice(inicio, inicio + porPaginaBD)
  }, [pagosBD, paginaBD, porPaginaBD])

  const manejarArchivo = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return

    const isExcel = file.name.endsWith('.xlsx') || file.name.endsWith('.xls')

    if (isExcel) {
      setImportando(true)
      setImportResultado(null)
      setImportError(null)
      try {
        const result = await invoke<ImportResult>('pick_and_import_excel')
        setImportResultado(result)
      } catch (err: any) {
        if (err !== 'No se selecciono ningun archivo') {
          setImportError(`Error al procesar Excel: ${err}`)
        }
      } finally {
        setImportando(false)
        e.target.value = ''
      }
    } else {
      setImportando(true)
      setImportResultado(null)
      setImportError(null)
      try {
        const contenido = await file.text()
        const result = await invoke<ImportResult>('import_csv', { contenido })
        setImportResultado(result)
      } catch (err: any) {
        setImportError(`Error al procesar CSV: ${err}`)
      } finally {
        setImportando(false)
        e.target.value = ''
      }
    }
  }

  const syncHistorico = async () => {
    if (!histFecha) return
    setHistSyncing(true)
    setHistResultado(null)
    setHistError(null)
    try {
      const result = await invoke<{ success: boolean; mensajes_nuevos: number; total_procesados: number; error?: string }>(
        'sync_historical',
        { sinceDate: histFecha },
      )
      if (result.success) {
        setHistResultado(
          `Sync completado: ${result.mensajes_nuevos} pago(s) nuevo(s) de ${result.total_procesados} correos procesados.`,
        )
        if (pagosBD) listarPagos()
      } else {
        setHistError(result.error || 'Error desconocido en sync historico.')
      }
    } catch (err: any) {
      setHistError(`Error en sync historico: ${err}`)
    } finally {
      setHistSyncing(false)
    }
  }

  return (
    <div className="max-w-4xl mx-auto">
      <div className="flex justify-between items-center mb-8 border-b border-slate-800 pb-4">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Binance Pay Auditor v1.4</h1>
          <p className="text-slate-400 text-sm">Conciliacion de pagos</p>
        </div>
        <SyncButton onSyncComplete={() => {}} />
      </div>

      {/* Historical Sync Section */}
      <div className="mb-8 bg-slate-800 p-6 rounded-xl border border-slate-700">
        <h2 className="text-lg font-semibold mb-2 text-amber-400">Sincronizacion Historica</h2>
        <p className="text-xs text-slate-500 mb-4">
          Busca TODOS los correos de Binance (no solo los no leidos) desde la fecha indicada.
          Los correos no se marcan como leidos. Los duplicados se saltan automaticamente.
        </p>
        <div className="flex gap-3 items-end">
          <div className="flex-1 min-w-[180px]">
            <label className="block text-xs font-medium text-slate-400 mb-1">Buscar desde</label>
            <input
              type="date"
              value={histFecha}
              onChange={(e) => setHistFecha(e.target.value)}
              className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-300 focus:outline-none focus:border-amber-500"
            />
          </div>
          <button
            onClick={syncHistorico}
            disabled={histSyncing || !histFecha}
            className="bg-purple-600 hover:bg-purple-700 disabled:bg-slate-700 disabled:cursor-not-allowed text-white font-medium px-6 py-2 rounded-lg text-sm transition-colors"
          >
            {histSyncing ? 'Sincronizando...' : 'Sincronizar Historico'}
          </button>
        </div>
        {histResultado && (
          <p className="mt-3 text-sm font-medium text-emerald-400">{histResultado}</p>
        )}
        {histError && (
          <p className="mt-3 text-sm text-rose-400">{histError}</p>
        )}
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-8">
        <VerificationForm />

        {/* CSV/Excel Import masivo */}
        <div className="bg-slate-800 p-6 rounded-xl border border-slate-700">
          <h2 className="text-lg font-semibold mb-4 text-slate-300">Carga Masiva de Reportes</h2>
          <p className="text-xs text-slate-500 mb-4">
            Subi un archivo <span className="text-slate-400 font-mono">.csv</span> o <span className="text-slate-400 font-mono">.xlsx</span> con columnas: <span className="text-slate-400 font-mono">usuario, monto, fecha</span>.
            Cada fila se verificara automaticamente contra los correos sincronizados.
          </p>

          <label className="block w-full cursor-pointer bg-slate-900 border-2 border-dashed border-slate-700 hover:border-amber-600/50 rounded-lg p-6 text-center transition-colors">
            <input
              type="file"
              accept=".csv,.xlsx,.xls"
              onChange={manejarArchivo}
              disabled={importando}
              className="hidden"
            />
            {importando ? (
              <p className="text-amber-400 text-sm font-medium">Procesando archivo...</p>
            ) : (
              <>
                <p className="text-slate-400 text-sm font-medium">Click para seleccionar archivo CSV o Excel</p>
                <p className="text-slate-600 text-xs mt-1">.csv, .xlsx, .xls</p>
              </>
            )}
          </label>

          {importError && (
            <p className="mt-3 text-sm text-rose-400">{importError}</p>
          )}

          {importResultado && (
            <div className="mt-4">
              {/* Summary cards */}
              <div className="grid grid-cols-4 gap-2 mb-4">
                <div className="bg-slate-900 rounded-lg p-3 text-center">
                  <p className="text-xs text-slate-500">Total</p>
                  <p className="text-lg font-bold text-slate-300">{importResultado.total_filas}</p>
                </div>
                <div className="bg-emerald-950/50 rounded-lg p-3 text-center">
                  <p className="text-xs text-emerald-500">Verificados</p>
                  <p className="text-lg font-bold text-emerald-400">{importResultado.verificados}</p>
                </div>
                <div className="bg-amber-950/50 rounded-lg p-3 text-center">
                  <p className="text-xs text-amber-500">No Encontrados</p>
                  <p className="text-lg font-bold text-amber-400">{importResultado.no_encontrados}</p>
                </div>
                <div className="bg-rose-950/50 rounded-lg p-3 text-center">
                  <p className="text-xs text-rose-500">Errores</p>
                  <p className="text-lg font-bold text-rose-400">{importResultado.errores}</p>
                </div>
              </div>

              {/* Detail table */}
              {importResultado.detalle.length > 0 && (
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
                      {importResultado.detalle.map((row) => (
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
                className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-xs text-slate-300 w-32 focus:outline-none focus:border-amber-500"
              />
            </div>
            <div>
              <label className="block text-[10px] font-medium text-slate-500 mb-0.5">Hasta</label>
              <input
                type="date"
                value={hastaBD}
                onChange={(e) => setHastaBD(e.target.value)}
                className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-xs text-slate-300 w-32 focus:outline-none focus:border-amber-500"
              />
            </div>
            <div>
              <label className="block text-[10px] font-medium text-slate-500 mb-0.5">Monto</label>
              <input
                type="number"
                step="0.01"
                placeholder="Exacto"
                value={montoExactoBD}
                onChange={(e) => setMontoExactoBD(e.target.value)}
                className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-xs w-20 focus:outline-none focus:border-amber-500"
              />
            </div>
            <div>
              <label className="block text-[10px] font-medium text-slate-500 mb-0.5">Min</label>
              <input
                type="number"
                step="0.01"
                placeholder="Min"
                value={montoMinBD}
                onChange={(e) => setMontoMinBD(e.target.value)}
                className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-xs w-20 focus:outline-none focus:border-amber-500"
              />
            </div>
            <div>
              <label className="block text-[10px] font-medium text-slate-500 mb-0.5">Max</label>
              <input
                type="number"
                step="0.01"
                placeholder="Max"
                value={montoMaxBD}
                onChange={(e) => setMontoMaxBD(e.target.value)}
                className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-xs w-20 focus:outline-none focus:border-amber-500"
              />
            </div>
            <button
              onClick={listarPagos}
              disabled={loadingBD}
              className="px-4 py-1.5 bg-amber-600 hover:bg-amber-700 disabled:bg-slate-700 text-white rounded text-xs font-medium transition-colors"
            >
              {loadingBD ? 'Cargando...' : (desdeBD && hastaBD ? 'Filtrar' : 'Listar Todos')}
            </button>
            {(desdeBD || hastaBD || montoExactoBD || montoMinBD || montoMaxBD) && (
              <button
                onClick={limpiarFiltros}
                className="px-4 py-1.5 bg-slate-700 hover:bg-slate-600 text-slate-300 rounded text-xs font-medium transition-colors"
                title="Limpiar todos los filtros"
              >
                Limpiar filtros
              </button>
            )}
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
                    <th className="pb-2 pr-2">ID</th>
                    <th className="pb-2 pr-2">Tipo</th>
                    <th className="pb-2 pr-2">Usuario</th>
                    <th className="pb-2 pr-2">Monto</th>
                    <th className="pb-2 pr-2">Moneda</th>
                    <th className="pb-2 pr-2">Fecha Correo</th>
                    <th className="pb-2 pr-2">Hora</th>
                    <th className="pb-2 pr-2">Estado</th>
                    <th className="pb-2 pr-2"></th>
                  </tr>
                </thead>
                <tbody>
                  {pagosPaginados.map((p) => (
                    <tr key={p.id} className="border-b border-slate-800 hover:bg-slate-800/50">
                      <td className="py-2 pr-2 text-slate-500">{p.id}</td>
                      <td className="py-2 pr-2">
                        <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${
                          p.tipo === 'deposito' ? 'bg-blue-950 text-blue-400' : 'bg-green-950 text-green-400'
                        }`}>
                          {p.tipo}
                        </span>
                      </td>
                      <td className="py-2 pr-2 font-mono text-slate-300">{p.usuario_remitente ?? '—'}</td>
                      <td className="py-2 pr-2 text-slate-300">{p.monto}</td>
                      <td className="py-2 pr-2 text-slate-400">{p.moneda}</td>
                      <td className="py-2 pr-2 text-slate-400">{formatFecha(p.fecha_correo)}</td>
                      <td className="py-2 pr-2 text-slate-400 font-mono text-xs">{p.hora_correo ?? '—'}</td>
                      <td className="py-2 pr-2">
                        <span className={`px-2 py-0.5 rounded-full text-xs font-medium ${
                          p.estado === 'verificado' ? 'bg-emerald-950 text-emerald-400' :
                          p.estado === 'por_revisar' ? 'bg-amber-950 text-amber-400' :
                          'bg-amber-950 text-amber-400'
                        }`}>
                          {p.estado}
                        </span>
                      </td>
                      <td className="py-2 pr-2">
                        {p.estado !== 'verificado' && (
                          <button
                            onClick={() => quickVerify(p.id)}
                            disabled={verificandoId === p.id}
                            className="px-2 py-0.5 bg-emerald-600 hover:bg-emerald-700 disabled:bg-slate-600 text-white rounded text-[10px] font-medium transition-colors"
                          >
                            {verificandoId === p.id ? '...' : '✓ Verificar'}
                          </button>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>

              <div className="flex items-center justify-between mt-4 pt-4 border-t border-slate-700">
                <div className="flex items-center gap-2 text-xs text-slate-400">
                  <span>Mostrar</span>
                  <select
                    value={porPaginaBD}
                    onChange={(e) => { setPorPaginaBD(Number(e.target.value)); setPaginaBD(1) }}
                    className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-xs focus:outline-none focus:border-amber-500"
                  >
                    <option value={10}>10</option>
                    <option value={20}>20</option>
                    <option value={50}>50</option>
                    <option value={100}>100</option>
                  </select>
                  <span>de {pagosBD.length}</span>
                </div>

                <div className="flex items-center gap-2">
                  <button
                    onClick={() => setPaginaBD(p => Math.max(1, p - 1))}
                    disabled={paginaBD === 1}
                    className="px-3 py-1 text-xs rounded bg-slate-800 border border-slate-700 text-slate-300 disabled:opacity-40 disabled:cursor-not-allowed hover:bg-slate-700 transition-colors"
                  >
                    Anterior
                  </button>
                  <span className="text-xs text-slate-400 min-w-[80px] text-center">
                    {paginaBD} / {totalPaginasBD}
                  </span>
                  <button
                    onClick={() => setPaginaBD(p => Math.min(totalPaginasBD, p + 1))}
                    disabled={paginaBD === totalPaginasBD}
                    className="px-3 py-1 text-xs rounded bg-slate-800 border border-slate-700 text-slate-300 disabled:opacity-40 disabled:cursor-not-allowed hover:bg-slate-700 transition-colors"
                  >
                    Siguiente
                  </button>
                </div>
              </div>

              <p className="mt-4 text-xs text-slate-500">
                Total: {pagosBD.length} pagos | Disponibles: {pagosBD.filter(p => p.estado === 'disponible').length} | Verificados: {pagosBD.filter(p => p.estado === 'verificado').length} | Por revisar: {pagosBD.filter(p => p.estado === 'por_revisar').length}
              </p>
            </div>
          )
        )}
      </div>
    </div>
  )
}
