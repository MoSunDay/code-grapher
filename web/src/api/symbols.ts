import type {
  CallGraph,
  EdgeDirection,
  Id,
  SymbolDetail,
  SymbolList,
  SymbolQueryParams,
} from './types'
import { apiClient } from './client'

export async function listSymbols(repoId: Id, params: SymbolQueryParams): Promise<SymbolList> {
  return (await apiClient.get<SymbolList>(`/repos/${repoId}/symbols`, { params })).data
}

export async function getSymbolDetail(symbolId: Id): Promise<SymbolDetail> {
  return (await apiClient.get<SymbolDetail>(`/symbols/${symbolId}`)).data
}

export async function getCallGraph(
  symbolId: Id,
  depth = 2,
  direction: EdgeDirection = 'both',
): Promise<CallGraph> {
  return (
    await apiClient.get<CallGraph>(`/symbols/${symbolId}/graph`, { params: { depth, direction } })
  ).data
}
