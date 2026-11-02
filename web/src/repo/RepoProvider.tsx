import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react'
import type { Id } from '../api/types'
import { useRepos } from '../hooks/queries'
import { RepoContext, readStoredRepoId, storeRepoId, type RepoContextValue } from './repo-context'

export function RepoProvider({ children }: { children: ReactNode }) {
  const [repoId, setRepoIdState] = useState<Id | null>(() => readStoredRepoId())
  const reposQuery = useRepos()
  const repos = useMemo(() => reposQuery.data?.repos ?? [], [reposQuery.data])

  const setRepoId = useCallback((id: Id) => {
    setRepoIdState(id)
    storeRepoId(id)
  }, [])

  useEffect(() => {
    if (repos.length === 0) return
    const stillValid = repoId !== null && repos.some((r) => r.id === repoId)
    if (stillValid) return
    const stored = readStoredRepoId()
    const fallback = stored !== null && repos.some((r) => r.id === stored) ? stored : repos[0].id
    setRepoId(fallback)
  }, [repos, repoId, setRepoId])

  const value = useMemo<RepoContextValue>(
    () => ({ repoId, setRepoId, repos, loading: reposQuery.isLoading }),
    [repoId, setRepoId, repos, reposQuery.isLoading],
  )

  return <RepoContext.Provider value={value}>{children}</RepoContext.Provider>
}
