import { useState, useEffect } from 'react'
import { api } from '../lib/api'

const COMPANY_GROUPS = [
  { value: 'ferreteria_principal', label: 'Ferretería Principal', color: 'bg-blue-950 text-blue-400' },
  { value: 'pintatodo', label: 'Pintatodo', color: 'bg-yellow-950 text-yellow-400' },
  { value: 'herramientas_brink', label: 'Herramientas Brink', color: 'bg-red-950 text-red-400' },
] as const

const companyGroupInfo = (val: string | null) =>
  COMPANY_GROUPS.find(g => g.value === val) ?? { value: '', label: '—', color: 'bg-slate-800 text-slate-500' }

interface UserRecord {
  id: string
  username: string
  role: string
  company_group: string | null
  station_name: string | null
  is_active: boolean
  created_at: string
}

export default function Users() {
  const [users, setUsers] = useState<UserRecord[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Create form state
  const [showCreate, setShowCreate] = useState(false)
  const [newUsername, setNewUsername] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [newRole, setNewRole] = useState('cashier')
  const [newStation, setNewStation] = useState('')
  const [newCompanyGroup, setNewCompanyGroup] = useState('')
  const [creating, setCreating] = useState(false)

  // Toggle state
  const [togglingId, setTogglingId] = useState<string | null>(null)

  const loadUsers = async () => {
    setLoading(true)
    setError(null)
    try {
      const result = await api.listUsers()
      setUsers(result)
    } catch (err: any) {
      setError(`Error al cargar usuarios: ${err}`)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    loadUsers()
  }, [])

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!newUsername || !newPassword) return
    setCreating(true)
    try {
      await api.createUser({
        username: newUsername,
        password: newPassword,
        role: newRole,
        company_group: newCompanyGroup || undefined,
        station_name: newStation || undefined,
      })
      setNewUsername('')
      setNewPassword('')
      setNewCompanyGroup('')
      setNewStation('')
      setShowCreate(false)
      loadUsers()
    } catch (err: any) {
      alert(`Error al crear usuario: ${err}`)
    } finally {
      setCreating(false)
    }
  }

  const handleToggleActive = async (user: UserRecord) => {
    const action = user.is_active ? 'desactivar' : 'activar'
    if (!window.confirm(`¿${action.charAt(0).toUpperCase() + action.slice(1)} al usuario "${user.username}"?`)) return
    setTogglingId(user.id)
    try {
      await api.updateUser(user.id, { is_active: !user.is_active })
      loadUsers()
    } catch (err: any) {
      alert(`Error al ${action} usuario: ${err}`)
    } finally {
      setTogglingId(null)
    }
  }

  // Admin-only guard is handled at route level in App.tsx

  return (
    <div className="max-w-4xl mx-auto">
      <div className="flex justify-between items-center mb-8 border-b border-slate-800 pb-4">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Usuarios</h1>
          <p className="text-slate-400 text-sm">Gestión de cuentas del sistema</p>
        </div>
        <button
          onClick={() => setShowCreate(!showCreate)}
          className="bg-amber-600 hover:bg-amber-700 text-white font-medium px-4 py-2 rounded-lg text-sm transition-colors"
        >
          {showCreate ? 'Cancelar' : '+ Nuevo Usuario'}
        </button>
      </div>

      {/* Create form */}
      {showCreate && (
        <form onSubmit={handleCreate} className="mb-8 bg-slate-800 p-6 rounded-xl border border-slate-700">
          <h2 className="text-lg font-semibold mb-4 text-slate-300">Crear Usuario</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1">Usuario</label>
              <input
                type="text"
                value={newUsername}
                onChange={(e) => setNewUsername(e.target.value)}
                required
                className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-300 focus:outline-none focus:border-amber-500"
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1">Contraseña</label>
              <input
                type="password"
                value={newPassword}
                onChange={(e) => setNewPassword(e.target.value)}
                required
                className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-300 focus:outline-none focus:border-amber-500"
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1">Rol</label>
              <select
                value={newRole}
                onChange={(e) => setNewRole(e.target.value)}
                className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-300 focus:outline-none focus:border-amber-500"
              >
                <option value="cashier">Cajera</option>
                <option value="admin">Admin</option>
              </select>
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1">Estación (opcional)</label>
              <input
                type="text"
                value={newStation}
                onChange={(e) => setNewStation(e.target.value)}
                placeholder="Ej: Caja 1"
                className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-300 focus:outline-none focus:border-amber-500"
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1">
                Grupo de Empresa {newRole === 'cashier' ? <span className="text-rose-400">*</span> : '(opcional)'}
              </label>
              <select
                value={newCompanyGroup}
                onChange={(e) => setNewCompanyGroup(e.target.value)}
                required={newRole === 'cashier'}
                className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-300 focus:outline-none focus:border-amber-500"
              >
                <option value="">— Sin grupo —</option>
                {COMPANY_GROUPS.map(g => (
                  <option key={g.value} value={g.value}>{g.label}</option>
                ))}
              </select>
            </div>
          </div>
          <button
            type="submit"
            disabled={creating}
            className="mt-4 bg-emerald-600 hover:bg-emerald-700 disabled:bg-slate-700 text-white font-medium px-6 py-2 rounded-lg text-sm transition-colors"
          >
            {creating ? 'Creando...' : 'Crear Usuario'}
          </button>
        </form>
      )}

      {/* Error */}
      {error && (
        <p className="mb-4 text-sm text-rose-400">{error}</p>
      )}

      {/* Loading */}
      {loading && (
        <p className="text-slate-500 text-sm">Cargando usuarios...</p>
      )}

      {/* Users Table */}
      {!loading && users.length === 0 && !error && (
        <p className="text-slate-500 text-sm">No hay usuarios registrados.</p>
      )}

      {!loading && users.length > 0 && (
        <div className="overflow-x-auto">
          <table className="w-full text-xs text-left">
            <thead className="text-slate-400 border-b border-slate-700">
              <tr>
                <th className="pb-2 pr-2">Usuario</th>
                <th className="pb-2 pr-2">Rol</th>
                <th className="pb-2 pr-2">Grupo</th>
                <th className="pb-2 pr-2">Estación</th>
                <th className="pb-2 pr-2">Estado</th>
                <th className="pb-2 pr-2">Creado</th>
                <th className="pb-2 pr-2">Acción</th>
              </tr>
            </thead>
            <tbody>
              {users.map((u) => (
                <tr key={u.id} className={`border-b border-slate-800 hover:bg-slate-800/50 ${!u.is_active ? 'opacity-50' : ''}`}>
                  <td className="py-2 pr-2 font-mono text-slate-300">{u.username}</td>
                  <td className="py-2 pr-2">
                    <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${
                      u.role === 'admin' ? 'bg-amber-950 text-amber-400' : 'bg-slate-800 text-slate-400'
                    }`}>
                      {u.role === 'admin' ? 'Admin' : 'Cajera'}
                    </span>
                  </td>
                  <td className="py-2 pr-2">
                    <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${companyGroupInfo(u.company_group).color}`}>
                      {companyGroupInfo(u.company_group).label}
                    </span>
                  </td>
                  <td className="py-2 pr-2 text-slate-400">{u.station_name ?? '—'}</td>
                  <td className="py-2 pr-2">
                    <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${
                      u.is_active ? 'bg-emerald-950 text-emerald-400' : 'bg-rose-950 text-rose-400'
                    }`}>
                      {u.is_active ? 'Activo' : 'Inactivo'}
                    </span>
                  </td>
                  <td className="py-2 pr-2 text-slate-500">{new Date(u.created_at).toLocaleDateString()}</td>
                  <td className="py-2 pr-2">
                    <button
                      onClick={() => handleToggleActive(u)}
                      disabled={togglingId === u.id}
                      className={`px-2 py-1 rounded text-[11px] font-medium transition-colors disabled:opacity-50 ${
                        u.is_active
                          ? 'bg-rose-900/50 hover:bg-rose-800 text-rose-400'
                          : 'bg-emerald-900/50 hover:bg-emerald-800 text-emerald-400'
                      }`}
                    >
                      {togglingId === u.id ? '...' : u.is_active ? 'Desactivar' : 'Activar'}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}
