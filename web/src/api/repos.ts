import type {
  AnalyzeResult,
  CreateRepoInput,
  Id,
  IndexTriggerResult,
  Repo,
  RepoList,
  RepoStatsPayload,
} from './types'
import { apiClient } from './client'

export async function listRepos(): Promise<RepoList> {
  return (await apiClient.get<RepoList>('/repos')).data
}

export async function createRepo(input: CreateRepoInput): Promise<Repo> {
  const res = await apiClient.post<{ repo: Repo }>('/repos', input)
  return res.data.repo
}

export async function triggerIndex(repoId: Id, full: boolean): Promise<IndexTriggerResult> {
  return (await apiClient.post<IndexTriggerResult>(`/repos/${repoId}/index`, { full })).data
}

export async function triggerAnalyze(repoId: Id): Promise<AnalyzeResult> {
  return (await apiClient.post<AnalyzeResult>(`/repos/${repoId}/analyze`)).data
}

export async function getRepoStats(repoId: Id): Promise<RepoStatsPayload> {
  return (await apiClient.get<RepoStatsPayload>(`/repos/${repoId}/stats`)).data
}
