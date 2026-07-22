import { invoke } from '@tauri-apps/api/core'

// Tipos locales para evitar dependencia circular
interface ApiResponse {
  token?: string
  refresh_token?: string
  user?: { id: string; username: string; role: string; station_name: string | null }
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
      user: { id: string; username: string; role: string; station_name: string | null; is_active: boolean; created_at: string }
    }>('client_login', { username, password }),

  setToken: (token: string) =>
    invoke<void>('client_set_token', { token }),

  verifyPayment: (usuarioEmpresa: string, montoEmpresa: number, fechaEmpresa: string) =>
    invoke<ApiResponse>('client_verify_payment', {
      usuarioEmpresa,
      montoEmpresa,
      fechaEmpresa,
    }),

  listPayments: (params?: {
    desde?: string
    hasta?: string
    monto_exacto?: number
    monto_min?: number
    monto_max?: number
  }) =>
    invoke<unknown[]>('client_list_payments', {
      desde: params?.desde || null,
      hasta: params?.hasta || null,
      montoExacto: params?.monto_exacto ?? null,
      montoMin: params?.monto_min ?? null,
      montoMax: params?.monto_max ?? null,
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
    invoke<Array<{ id: string; username: string; role: string; station_name: string | null; is_active: boolean; created_at: string }>>('client_list_users'),

  createUser: (data: { username: string; password: string; role: string; station_name?: string }) =>
    invoke<{ id: string; username: string; role: string; station_name: string | null; is_active: boolean; created_at: string }>('client_create_user', data),

  updateUser: (id: string, data: { is_active?: boolean; role?: string; station_name?: string }) =>
    invoke<{ id: string; username: string; role: string; station_name: string | null; is_active: boolean; created_at: string }>('client_update_user', { userId: id, updates: data }),
}
