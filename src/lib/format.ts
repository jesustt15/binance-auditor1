/**
 * Convierte una fecha RFC3339 (2026-07-10T16:16:07+00:00) a DD/MM/AAAA
 * Usa UTC para evitar cambios de zona horaria
 */
export function formatFecha(rfc3339: string): string {
  const d = new Date(rfc3339)
  if (isNaN(d.getTime())) return rfc3339 // fallback si no es fecha válida
  const dd = String(d.getUTCDate()).padStart(2, '0')
  const mm = String(d.getUTCMonth() + 1).padStart(2, '0')
  const aaaa = d.getUTCFullYear()
  return `${dd}/${mm}/${aaaa}`
}
