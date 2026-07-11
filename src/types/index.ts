export interface PagoBinance {
  id: number
  usuario_remitente: string
  monto: number
  moneda: string
  fecha_correo: string
  estado: 'disponible' | 'verificado'
  observaciones: string | null
  verificado_en: string | null
  creado_en: string
}

export interface SyncResult {
  success: boolean
  mensajes_nuevos: number
  error?: string
}

export interface VerifyResult {
  verificado: boolean
  mensaje: string
  data?: PagoBinance
}

export interface AppSettings {
  imap_user: string
  imap_password: string
}
