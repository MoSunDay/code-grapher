import type { Id, TaskList, TaskStatus, UnresolvedEdgeList } from './types'
import { apiClient } from './client'

export async function listTasks(repoId: Id, status?: TaskStatus): Promise<TaskList> {
  return (
    await apiClient.get<TaskList>(`/repos/${repoId}/tasks`, { params: { status, limit: 100 } })
  ).data
}

export async function retryTask(taskId: Id): Promise<{ ok: boolean }> {
  return (await apiClient.post<{ ok: boolean }>(`/tasks/${taskId}/retry`)).data
}

export async function listUnresolvedEdges(repoId: Id): Promise<UnresolvedEdgeList> {
  return (await apiClient.get<UnresolvedEdgeList>(`/repos/${repoId}/edges/unresolved`)).data
}

export async function confirmEdge(edgeId: Id): Promise<{ ok: boolean }> {
  return (await apiClient.post<{ ok: boolean }>(`/edges/${edgeId}/confirm`)).data
}

export async function dismissEdge(edgeId: Id): Promise<{ ok: boolean }> {
  return (await apiClient.post<{ ok: boolean }>(`/edges/${edgeId}/dismiss`)).data
}
