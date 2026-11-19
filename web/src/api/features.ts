import { apiClient } from './client'
import type {
  BlobContent,
  FeatureApproveResult,
  FeatureDiff,
  FeatureTree,
  MrCommits,
  MrList,
} from './types'

export async function listMrs(featureRepoId: string): Promise<MrList> {
  return (await apiClient.get<MrList>(`/features/${featureRepoId}/mrs`)).data
}

export async function getTree(featureRepoId: string, mr?: string): Promise<FeatureTree> {
  const query = mr === undefined || mr === '' ? '' : `?mr=${encodeURIComponent(mr)}`
  return (await apiClient.get<FeatureTree>(`/features/${featureRepoId}/tree${query}`)).data
}

export async function getBlob(
  featureRepoId: string,
  path: string,
  mr?: string,
): Promise<BlobContent> {
  const mrQuery = mr === undefined || mr === '' ? '' : `&mr=${encodeURIComponent(mr)}`
  return (
    await apiClient.get<BlobContent>(
      `/features/${featureRepoId}/blob?path=${encodeURIComponent(path)}${mrQuery}`,
    )
  ).data
}

export async function getMrDiff(featureRepoId: string, name: string): Promise<FeatureDiff> {
  return (
    await apiClient.get<FeatureDiff>(`/features/${featureRepoId}/${encodeURIComponent(name)}/diff`)
  ).data
}

export async function getMrCommits(featureRepoId: string, name: string): Promise<MrCommits> {
  return (
    await apiClient.get<MrCommits>(
      `/features/${featureRepoId}/${encodeURIComponent(name)}/commits`,
    )
  ).data
}

export async function approveMr(
  featureRepoId: string,
  name: string,
  body: { reviewer: string; note?: string; expected_tip?: string },
): Promise<FeatureApproveResult> {
  return (
    await apiClient.post<FeatureApproveResult>(
      `/features/${featureRepoId}/${encodeURIComponent(name)}/approve`,
      body,
    )
  ).data
}

export interface ChangelogEntry {
  file: string
  markdown: string
}

export interface ChangelogList {
  repo_id: string
  changelog: ChangelogEntry[]
}

export async function listChangelog(featureRepoId: string): Promise<ChangelogList> {
  return (await apiClient.get<ChangelogList>(`/features/${featureRepoId}/changelog`)).data
}
