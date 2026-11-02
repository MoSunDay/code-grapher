import { useQuery } from '@tanstack/react-query'
import type { Id, SymbolQueryParams, TaskStatus } from '../api/types'
import { getConfig } from '../api/config'
import { getFileContent, listFiles } from '../api/files'
import { getRepoStats, listRepos } from '../api/repos'
import { getCallGraph, getSymbolDetail, listSymbols } from '../api/symbols'
import { listTasks, listUnresolvedEdges } from '../api/tasks'

export function requireRepoId(repoId: Id | null): Id {
  if (repoId === null) throw new Error('尚未选择仓库')
  return repoId
}

export function useRepos() {
  return useQuery({ queryKey: ['repos'], queryFn: listRepos })
}

export function useRepoStats(repoId: Id | null, refetchInterval: number | false = false) {
  return useQuery({
    queryKey: ['repos', repoId, 'stats'],
    queryFn: () => getRepoStats(requireRepoId(repoId)),
    enabled: repoId !== null,
    refetchInterval,
  })
}

export function useFiles(repoId: Id | null) {
  return useQuery({
    queryKey: ['repos', repoId, 'files'],
    queryFn: () => listFiles(requireRepoId(repoId)),
    enabled: repoId !== null,
  })
}

export function useFileContent(repoId: Id | null, path: string | null) {
  return useQuery({
    queryKey: ['repos', repoId, 'file', path],
    queryFn: () => getFileContent(requireRepoId(repoId), requirePath(path)),
    enabled: repoId !== null && path !== null,
  })
}

export function useSymbols(repoId: Id | null, params: SymbolQueryParams | null) {
  return useQuery({
    queryKey: ['repos', repoId, 'symbols', params],
    queryFn: () => listSymbols(requireRepoId(repoId), params ?? {}),
    enabled: repoId !== null && params !== null,
  })
}

export function useSymbolDetail(symbolId: Id | null) {
  return useQuery({
    queryKey: ['symbols', symbolId],
    queryFn: () => getSymbolDetail(requireId(symbolId)),
    enabled: symbolId !== null,
  })
}

export function useCallGraph(symbolId: Id | null, depth = 2) {
  return useQuery({
    queryKey: ['symbols', symbolId, 'graph', depth],
    queryFn: () => getCallGraph(requireId(symbolId), depth),
    enabled: symbolId !== null,
  })
}

export function useTasks(repoId: Id | null, status?: TaskStatus) {
  return useQuery({
    queryKey: ['repos', repoId, 'tasks', status ?? 'all'],
    queryFn: () => listTasks(requireRepoId(repoId), status),
    enabled: repoId !== null,
    refetchInterval: (query) => {
      const tasks = query.state.data?.tasks ?? []
      return tasks.some((t) => t.status === 'pending' || t.status === 'running') ? 3000 : false
    },
  })
}

export function useUnresolvedEdges(repoId: Id | null) {
  return useQuery({
    queryKey: ['repos', repoId, 'edges', 'unresolved'],
    queryFn: () => listUnresolvedEdges(requireRepoId(repoId)),
    enabled: repoId !== null,
  })
}

export function useConfig() {
  return useQuery({ queryKey: ['config'], queryFn: getConfig, staleTime: 60_000 })
}

function requireId(id: Id | null): Id {
  if (id === null) throw new Error('缺少符号 ID')
  return id
}

function requirePath(path: string | null): string {
  if (path === null) throw new Error('缺少文件路径')
  return path
}
