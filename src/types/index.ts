export interface PagoBinance {
  id: number
  tipo: 'pago' | 'deposito'
  usuario_remitente: string | null
  monto: number
  moneda: string
  fecha_correo: string
  estado: 'disponible' | 'verificado' | 'por_revisar'
  observaciones: string | null
  verificado_en: string | null
  creado_en: string
}

export interface SyncResult {
  success: boolean
  mensajes_nuevos: number
  total_procesados: number
  error?: string
}

export interface SyncProgress {
  actual: number
  total: number
  nuevos: number
}

export interface VerifyResult {
  verificado: boolean
  mensaje: string
  data?: PagoBinance
}

export interface ExportResult {
  success: boolean
  file_path: string
  mensaje: string
}

export interface ImportRowDetail {
  fila: number
  usuario: string
  monto: number
  fecha: string
  resultado: string
}

export interface ImportResult {
  total_filas: number
  verificados: number
  no_encontrados: number
  errores: number
  detalle: ImportRowDetail[]
}

export interface AppSettings {
  imap_user: string
  imap_password: string
}
