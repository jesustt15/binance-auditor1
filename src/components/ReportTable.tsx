import { useState, useMemo } from 'react'
import type { PagoBinance } from '../types'
import { formatFecha } from '../lib/format'

interface Props {
  pagos: PagoBinance[]
}

export default function ReportTable({ pagos }: Props) {
  const [pagina, setPagina] = useState(1)
  const [porPagina, setPorPagina] = useState(20)

  const totalPaginas = useMemo(() => Math.ceil(pagos.length / porPagina), [pagos.length, porPagina])

  const pagosPaginados = useMemo(() => {
    const inicio = (pagina - 1) * porPagina
    return pagos.slice(inicio, inicio + porPagina)
  }, [pagos, pagina, porPagina])

  if (pagos.length === 0) {
    return (
      <div className="text-center text-slate-500 py-8 text-sm">
        No hay pagos en el rango de fechas seleccionado.
      </div>
    )
  }

  const totalVerificados = pagos.filter(p => p.estado === 'verificado').length
  const montoTotal = pagos.reduce((sum, p) => sum + p.monto, 0)

  return (
    <div>
      <div className="grid grid-cols-3 gap-4 mb-6">
        <div className="bg-slate-800 p-4 rounded-lg border border-slate-700">
          <p className="text-xs text-slate-400">Total Pagos</p>
          <p className="text-xl font-bold text-slate-100">{pagos.length}</p>
        </div>
        <div className="bg-slate-800 p-4 rounded-lg border border-slate-700">
          <p className="text-xs text-slate-400">Verificados</p>
          <p className="text-xl font-bold text-emerald-400">{totalVerificados}</p>
        </div>
        <div className="bg-slate-800 p-4 rounded-lg border border-slate-700">
          <p className="text-xs text-slate-400">Monto Total</p>
          <p className="text-xl font-bold text-amber-400">{montoTotal.toFixed(2)} USDT</p>
        </div>
      </div>

      {/* Leyenda de grupos empresariales */}
      <div className="flex items-center gap-4 mb-3 text-[10px] text-slate-500">
        <span>Grupos:</span>
        <span className="flex items-center gap-1">
          <span className="inline-block w-2.5 h-2.5 rounded-sm bg-blue-500/60" />
          Ferretería Principal
        </span>
        <span className="flex items-center gap-1">
          <span className="inline-block w-2.5 h-2.5 rounded-sm bg-yellow-500/60" />
          Pintatodo
        </span>
        <span className="flex items-center gap-1">
          <span className="inline-block w-2.5 h-2.5 rounded-sm bg-red-500/60" />
          Herramientas Brink
        </span>
      </div>

      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-slate-700 text-slate-400 text-xs uppercase">
              <th className="text-left py-2 px-3">ID</th>
              <th className="text-left py-2 px-3">Tipo</th>
              <th className="text-left py-2 px-3">Usuario</th>
              <th className="text-right py-2 px-3">Monto</th>
              <th className="text-left py-2 px-3">Fecha Correo</th>
              <th className="text-left py-2 px-3">Hora</th>
              <th className="text-center py-2 px-3">Estado</th>
              <th className="text-left py-2 px-3">Verificado Por</th>
              <th className="text-left py-2 px-3">Observaciones</th>
            </tr>
          </thead>
          <tbody>
            {pagosPaginados.map((pago) => (
              <tr key={pago.id} className="border-b border-slate-800 hover:bg-slate-800/50">
                <td className="py-2 px-3 text-slate-500">{pago.id}</td>
                <td className="py-2 px-3">
                  <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${
                    pago.tipo === 'deposito'
                      ? 'bg-blue-900/50 text-blue-400 border border-blue-700'
                      : 'bg-green-900/50 text-green-400 border border-green-700'
                  }`}>
                    {pago.tipo}
                  </span>
                </td>
                <td className="py-2 px-3 text-slate-200">{pago.usuario_remitente ?? '—'}</td>
                <td className="py-2 px-3 text-right text-slate-200 font-mono">
                  {pago.monto.toFixed(2)} {pago.moneda}
                </td>
                <td className="py-2 px-3 text-slate-400 text-xs">
                  {formatFecha(pago.fecha_correo)}
                </td>
                <td className="py-2 px-3 text-slate-500 text-xs">
                  {pago.hora_correo ?? '—'}
                </td>
                <td className="py-2 px-3 text-center">
                  <span className={`px-2 py-0.5 rounded-full text-xs font-medium ${
                    pago.estado === 'verificado'
                      ? 'bg-emerald-900/50 text-emerald-400 border border-emerald-700'
                      : pago.estado === 'por_revisar'
                      ? 'bg-amber-900/50 text-amber-400 border border-amber-700'
                      : 'bg-amber-900/50 text-amber-400 border border-amber-700'
                  }`}>
                    {pago.estado}
                  </span>
                </td>
                <td className="py-2 px-3 text-slate-500 text-xs">
                  <span className="flex items-center gap-1">
                    <span>{pago.verified_by_name ?? '—'}</span>
                    {pago.company_group ? (
                      <span className={`px-1 py-0.5 rounded text-[9px] font-medium ${
                        pago.company_group === 'ferreteria_principal' ? 'bg-blue-950 text-blue-400' :
                        pago.company_group === 'pintatodo' ? 'bg-yellow-950 text-yellow-400' :
                        'bg-red-950 text-red-400'
                      }`}>
                        {pago.company_group === 'ferreteria_principal' ? 'Ferretería Principal' :
                         pago.company_group === 'pintatodo' ? 'Pintatodo' :
                         'Herramientas Brink'}
                      </span>
                    ) : null}
                  </span>
                </td>
                <td className="py-2 px-3 text-slate-500 text-xs max-w-xs truncate">
                  {pago.observaciones || '-'}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="flex items-center justify-between mt-4 pt-4 border-t border-slate-700">
        <div className="flex items-center gap-2 text-xs text-slate-400">
          <span>Mostrar</span>
          <select
            value={porPagina}
            onChange={(e) => { setPorPagina(Number(e.target.value)); setPagina(1) }}
            className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-xs focus:outline-none focus:border-amber-500"
          >
            <option value={10}>10</option>
            <option value={20}>20</option>
            <option value={50}>50</option>
            <option value={100}>100</option>
          </select>
          <span>de {pagos.length}</span>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={() => setPagina(p => Math.max(1, p - 1))}
            disabled={pagina === 1}
            className="px-3 py-1 text-xs rounded bg-slate-800 border border-slate-700 text-slate-300 disabled:opacity-40 disabled:cursor-not-allowed hover:bg-slate-700 transition-colors"
          >
            Anterior
          </button>
          <span className="text-xs text-slate-400 min-w-20 text-center">
            {pagina} / {totalPaginas}
          </span>
          <button
            onClick={() => setPagina(p => Math.min(totalPaginas, p + 1))}
            disabled={pagina === totalPaginas}
            className="px-3 py-1 text-xs rounded bg-slate-800 border border-slate-700 text-slate-300 disabled:opacity-40 disabled:cursor-not-allowed hover:bg-slate-700 transition-colors"
          >
            Siguiente
          </button>
        </div>
      </div>
    </div>
  )
}
