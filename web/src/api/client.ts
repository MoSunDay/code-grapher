import axios from 'axios'
import { notify } from '../lib/notify'

export const apiClient = axios.create({ baseURL: '/api', timeout: 30_000 })

export function extractErrorMessage(error: unknown): string {
  if (axios.isAxiosError(error)) {
    const detail = readErrorDetail(error.response?.data)
    if (detail !== null) return detail
    if (error.code === 'ECONNABORTED') return '请求超时'
    if (error.response !== undefined)
      return `HTTP ${error.response.status} ${error.response.statusText}`
    return '无法连接后端服务（dev 代理指向 http://127.0.0.1:7800）'
  }
  return error instanceof Error ? error.message : '未知错误'
}

function readErrorDetail(data: unknown): string | null {
  if (typeof data === 'string' && data.trim() !== '') return data.slice(0, 300)
  if (typeof data === 'object' && data !== null) {
    const record = data as Record<string, unknown>
    for (const key of ['error', 'message', 'detail']) {
      const value = record[key]
      if (typeof value === 'string' && value !== '') return value.slice(0, 300)
    }
  }
  return null
}

apiClient.interceptors.response.use(
  (response) => response,
  (error: unknown) => {
    const method = axios.isAxiosError(error) ? (error.config?.method ?? '') : ''
    if (method !== '' && method !== 'get') notify.error(extractErrorMessage(error))
    return Promise.reject(error instanceof Error ? error : new Error(extractErrorMessage(error)))
  },
)
