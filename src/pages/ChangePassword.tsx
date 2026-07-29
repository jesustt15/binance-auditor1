import { useState, type FormEvent } from 'react'
import { useAuth } from '../App'
import { api } from '../lib/api'
import { useToast } from '../components/Toast'

interface Props {
  onPasswordChanged: () => void
}

export default function ChangePassword({ onPasswordChanged }: Props) {
  const { auth } = useAuth()
  const [currentPassword, setCurrentPassword] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [loading, setLoading] = useState(false)
  const [success, setSuccess] = useState(false)
  const { toast } = useToast()

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()

    if (newPassword.length < 8) {
      toast('La contraseña debe tener al menos 8 caracteres', 'warning')
      return
    }
    // Check complexity
    const hasUpper = /[A-Z]/.test(newPassword)
    const hasLower = /[a-z]/.test(newPassword)
    const hasDigit = /\d/.test(newPassword)
    const hasSpecial = /[^A-Za-z0-9]/.test(newPassword)
    if (!hasUpper || !hasLower || !hasDigit || !hasSpecial) {
      toast('La contraseña debe incluir mayúsculas, minúsculas, números y caracteres especiales', 'warning')
      return
    }

    if (newPassword !== confirmPassword) {
      toast('Las contraseñas no coinciden', 'warning')
      return
    }

    setLoading(true)

    try {
      await api.changePassword(currentPassword, newPassword)
      setSuccess(true)
      toast('¡Contraseña actualizada!', 'success')
      setTimeout(() => onPasswordChanged(), 1500)
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err ?? 'Error al cambiar la contraseña')
      toast(msg, 'error')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="min-h-screen bg-slate-900 flex items-center justify-center">
      <div className="bg-slate-800 p-8 rounded-xl border border-slate-700 w-full max-w-md">
        <div className="text-center mb-8">
          <svg className="mx-auto mb-4" width="48" height="48" viewBox="0 0 48 46" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M25.946 44.938c-.664.845-2.021.375-2.021-.698V33.937a2.26 2.26 0 0 0-2.262-2.262H10.287c-.92 0-1.456-1.04-.92-1.788l7.48-10.471c1.07-1.497 0-3.578-1.842-3.578H1.237c-.92 0-1.456-1.04-.92-1.788L10.013.474c.214-.297.556-.474.92-.474h28.894c.92 0 1.456 1.04.92 1.788l-7.48 10.471c-1.07 1.498 0 3.579 1.842 3.579h11.377c.943 0 1.473 1.088.89 1.83L25.947 44.94z" fill="#f59e0b"/>
          </svg>
          <h1 className="text-xl font-bold text-slate-100">Cambiar Contraseña</h1>
          <p className="text-slate-400 text-sm mt-2">
            {success
              ? 'Contraseña actualizada. Redirigiendo...'
              : 'Por seguridad, debes cambiar tu contraseña antes de continuar'}
          </p>
        </div>

        {!success && (
          <form onSubmit={handleSubmit} className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-slate-300 mb-2">
                Contraseña actual
              </label>
              <input
                type="password"
                required
                value={currentPassword}
                onChange={(e) => setCurrentPassword(e.target.value)}
                className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-amber-500 text-slate-200"
                placeholder="••••••••"
              />
            </div>

            <div>
              <label className="block text-sm font-medium text-slate-300 mb-2">
                Nueva contraseña
              </label>
              <input
                type="password"
                required
                value={newPassword}
                onChange={(e) => setNewPassword(e.target.value)}
                className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-amber-500 text-slate-200"
                placeholder="8+ caracteres, mayúsculas, minúsculas, números y especiales"
              />
              <p className="text-[10px] text-slate-500 mt-1">Mínimo 8 caracteres con mayúsculas, minúsculas, números y símbolos</p>
            </div>

            <div>
              <label className="block text-sm font-medium text-slate-300 mb-2">
                Confirmar nueva contraseña
              </label>
              <input
                type="password"
                required
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-amber-500 text-slate-200"
                placeholder="repetir contraseña"
              />
            </div>

            <button
              type="submit"
              disabled={loading}
              className="w-full bg-amber-500 hover:bg-amber-600 disabled:bg-slate-700 disabled:cursor-not-allowed text-slate-950 font-medium py-2 rounded-lg text-sm transition-colors"
            >
              {loading ? 'Cambiando...' : 'Cambiar Contraseña'}
            </button>
          </form>
        )}

        {success && (
          <div className="text-center">
            <div className="w-14 h-14 mx-auto mb-3 rounded-full bg-emerald-500/20 flex items-center justify-center">
              <svg className="w-7 h-7 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M5 13l4 4L19 7" />
              </svg>
            </div>
            <p className="text-emerald-400 text-sm font-medium">¡Contraseña actualizada con éxito!</p>
          </div>
        )}
      </div>
    </div>
  )
}
