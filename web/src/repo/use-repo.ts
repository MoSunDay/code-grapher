import { useContext } from 'react'
import { RepoContext } from './repo-context'

export function useRepo() {
  const ctx = useContext(RepoContext)
  if (ctx === null) throw new Error('useRepo 必须在 RepoProvider 内使用')
  return ctx
}

export function useSelectedRepo() {
  const { repos, repoId } = useRepo()
  return repos.find((r) => r.id === repoId) ?? null
}
