import { invoke } from '@tauri-apps/api/core'

// Tipo compartido de User para respuestas
export interface ApiUser {
  id: string
  username: string
  role: string
  company_group: string | null
  station_name: string | null
  is_active: boolean
  must_change_password: boolean
  created_at: string
}

// Tipos locales para evitar dependencia circular
interface ApiResponse {
  token?: string
  refresh_token?: string
  user?: ApiUser
  verificado?: boolean
  mensaje?: string
  data?: unknown
  [key: string]: unknown
}

/**
 * Wrapper tipado para todas las llamadas REST desde modo cliente.
 * En modo standalone, usa invoke normal de Tauri.
 * En modo client, el backend de Rust hace proxying a la API HTTP.
 */
export const api = {
  login: (username: string, password: string) =>
    invoke<{
      token: string
      refresh_token: string
      user: ApiUser
    }>('client_login', { username, password }),

  verifyPayment: (usuarioEmpresa: string, montoEmpresa: number, fechaEmpresa: string) =>
    invoke<ApiResponse>('client_verify_payment', {
      usuarioEmpresa,
      montoEmpresa,
      fechaEmpresa,
    }),

  listPayments: (desde?: string, hasta?: string) =>
    invoke<unknown[]>('client_list_payments', {
      desde: desde || null,
      hasta: hasta || null,
    }),

  triggerSync: (mode?: string, sinceDate?: string) =>
    invoke<ApiResponse>('client_trigger_sync', {
      mode: mode || 'normal',
      sinceDate: sinceDate || null,
    }),

  getImapConfig: () =>
    invoke<{ email: string; imap_host: string; imap_port: number; last_sync_at: string | null }>(
      'client_get_imap_config',
    ),

  saveImapConfig: (email: string, password: string, host?: string, port?: number) =>
    invoke<{ success: boolean }>('client_save_imap_config', {
      email,
      password,
      host: host || 'imap.gmail.com',
      port: port || 993,
    }),

  getAuditLog: (params?: Record<string, string>) =>
    invoke<unknown[]>('client_audit_log', { params: params || {} }),

  listQuarantine: (status?: string) =>
    invoke<unknown[]>('client_list_quarantine', { status: status || 'pending' }),

  reviewQuarantine: (id: string, decision: string) =>
    invoke<{ success: boolean }>('client_review_quarantine', { id, decision }),

  quickVerify: (id: string) =>
    invoke<{ verificado: boolean; mensaje: string; data: unknown }>('client_quick_verify_pago', { paymentId: id }),

  listUsers: () =>
    invoke<ApiUser[]>('client_list_users'),

  createUser: (data: { username: string; password: string; role: string; company_group?: string; station_name?: string }) =>
    invoke<ApiUser>('client_create_user', data),

  updateUser: (id: string, data: { username?: string; is_active?: boolean; role?: string; company_group?: string; station_name?: string }) =>
    invoke<ApiUser>('client_update_user', { userId: id, updates: data }),

  deleteUser: (id: string) =>
    invoke<{ success: boolean }>('client_delete_user', { userId: id }),

  adminResetPassword: (id: string, newPassword: string) =>
    invoke<{ success: boolean; mensaje: string }>('client_admin_reset_password', { userId: id, newPassword }),

  changePassword: (currentPassword: string, newPassword: string) =>
    invoke<{ success: boolean; mensaje: string }>('client_change_password', {
      currentPassword,
      newPassword,
    }),
}
