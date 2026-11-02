import dayjs from 'dayjs'

export function fmtNumber(value: number | null | undefined): string {
  return typeof value === 'number' && Number.isFinite(value) ? value.toLocaleString('zh-CN') : '-'
}

export function fmtTime(value: string | number | null | undefined): string {
  if (value === null || value === undefined || value === '') return '-'
  const asNumber = typeof value === 'number' ? value : Number(value)
  if (Number.isFinite(asNumber) && `${value}`.trim() !== '') {
    const ms = asNumber < 1e12 ? asNumber * 1000 : asNumber
    return dayjs(ms).format('YYYY-MM-DD HH:mm:ss')
  }
  const parsed = dayjs(value)
  return parsed.isValid() ? parsed.format('YYYY-MM-DD HH:mm:ss') : String(value)
}

export function fmtDuration(ms: number | null | undefined): string {
  if (typeof ms !== 'number' || !Number.isFinite(ms)) return '-'
  if (ms < 1000) return `${Math.round(ms)}ms`
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`
  return `${Math.floor(ms / 60_000)}m${Math.round((ms % 60_000) / 1000)}s`
}
