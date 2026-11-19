import type { ServerConfig, UpdateConfigResult } from './types'
import { apiClient } from './client'

export async function getConfig(): Promise<ServerConfig> {
  return (await apiClient.get<ServerConfig>('/config')).data
}

export async function updateConfig(analyzeConcurrency: number): Promise<UpdateConfigResult> {
  return (
    await apiClient.put<UpdateConfigResult>('/config', {
      analyze_concurrency: analyzeConcurrency,
    })
  ).data
}
