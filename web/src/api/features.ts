import type {
  ChangelogList,
  FeatureApproveResult,
  FeatureDetail,
  FeatureDiff,
  FeatureDraftResult,
  FeatureList,
} from './types'
import { apiClient } from './client'

export async function listFeatures(featureRepoId: string): Promise<FeatureList> {
  return (await apiClient.get<FeatureList>(`/features/${featureRepoId}`)).data
}

export async function getFeature(
  featureRepoId: string,
  name: string,
): Promise<FeatureDetail> {
  return (
    await apiClient.get<FeatureDetail>(
      `/features/${featureRepoId}/${encodeURIComponent(name)}`,
    )
  ).data
}

export async function getFeatureDiff(
  featureRepoId: string,
  name: string,
): Promise<FeatureDiff> {
  return (
    await apiClient.get<FeatureDiff>(
      `/features/${featureRepoId}/${encodeURIComponent(name)}/diff`,
    )
  ).data
}

export async function createFeatureDraft(
  featureRepoId: string,
  feature: Record<string, unknown>,
): Promise<FeatureDraftResult> {
  return (
    await apiClient.post<FeatureDraftResult>(`/features/${featureRepoId}`, feature)
  ).data
}

export async function approveFeature(
  featureRepoId: string,
  name: string,
  input: { reviewer: string; note?: string; expected_hash?: string },
): Promise<FeatureApproveResult> {
  return (
    await apiClient.post<FeatureApproveResult>(
      `/features/${featureRepoId}/${encodeURIComponent(name)}/approve`,
      input,
    )
  ).data
}

export async function listChangelog(featureRepoId: string): Promise<ChangelogList> {
  return (await apiClient.get<ChangelogList>(`/features/${featureRepoId}/changelog`)).data
}
