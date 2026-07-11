import { useState } from 'react'
import SyncButton from '../components/SyncButton'
import VerificationForm from '../components/VerificationForm'

export default function Dashboard() {
  const [syncCount, setSyncCount] = useState(0)

  return (
    <div className="max-w-4xl mx-auto">
      <div className="flex justify-between items-center mb-8 border-b border-slate-800 pb-4">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Binance Pay Auditor v1.0</h1>
          <p className="text-slate-400 text-sm">Conciliacion de pagos</p>
        </div>
        <SyncButton onSyncComplete={() => setSyncCount(c => c + 1)} />
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-8">
        <VerificationForm />

        <div className="flex flex-col justify-between bg-slate-800 p-6 rounded-xl border border-slate-700">
          <div>
            <h2 className="text-lg font-semibold mb-4 text-slate-300">Estado de la Validacion</h2>
            <div className="border-2 border-dashed border-slate-700 rounded-lg p-8 text-center text-slate-500 text-sm">
              Introduce los datos del formulario para ejecutar la auditoria automatica.
              <p className="mt-2 text-xs text-slate-600">
                Pagos disponibles en base local: sincroniza correos primero
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
