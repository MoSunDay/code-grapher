import { createContext } from 'react'
import type { Id, Repo } from '../api/types'

export interface RepoContextValue {
  repoId: Id | null
  setRepoId: (id: Id) => void
  repos: Repo[]
  loading: boolean
}

export const RepoContext = createContext<RepoContextValue | null>(null)

const STORAGE_KEY = 'cg:repoId'

export function readStoredRepoId(): Id | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw === null || raw === '') return null
    const n = Number(raw)
    return Number.isFinite(n) && String(n) === raw ? n : raw
  } catch {
    return null
  }
}

export function storeRepoId(id: Id): void {
  try {
    localStorage.setItem(STORAGE_KEY, String(id))
  } catch {
    /* localStorage 不可用时忽略 */
  }
}
