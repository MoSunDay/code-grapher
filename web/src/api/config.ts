import type { ServerConfig } from './types'
import { apiClient } from './client'

export async function getConfig(): Promise<ServerConfig> {
  return (await apiClient.get<ServerConfig>('/config')).data
}
