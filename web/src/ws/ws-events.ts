import type { Id, WsEvent } from '../api/types'

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null

function asId(v: unknown): Id | null {
  return typeof v === 'number' || typeof v === 'string' ? v : null
}

function asNum(v: unknown): number | null {
  return typeof v === 'number' && Number.isFinite(v) ? v : null
}

function asStr(v: unknown, fallback = ''): string {
  return typeof v === 'string' ? v : fallback
}

export function wsEventsUrl(): string {
  const proto = window.location.protocol === 'https:' ? 'wss' : 'ws'
  return `${proto}://${window.location.host}/ws/events`
}

export function parseWsEvent(raw: unknown): WsEvent | null {
  if (!isRecord(raw)) return null
  switch (raw.type) {
    case 'index_progress': {
      const repoId = asId(raw.repo_id)
      const runId = asId(raw.run_id)
      const done = asNum(raw.done)
      const total = asNum(raw.total)
      if (repoId === null || runId === null || done === null || total === null) return null
      return {
        type: 'index_progress',
        repo_id: repoId,
        run_id: runId,
        done,
        total,
        phase: asStr(raw.phase),
      }
    }
    case 'index_done': {
      const repoId = asId(raw.repo_id)
      const runId = asId(raw.run_id)
      const stats = isRecord(raw.stats) ? raw.stats : null
      if (repoId === null || runId === null || stats === null) return null
      return {
        type: 'index_done',
        repo_id: repoId,
        run_id: runId,
        stats: {
          files_changed: asNum(stats.files_changed) ?? 0,
          symbols: asNum(stats.symbols) ?? 0,
          edges: asNum(stats.edges) ?? 0,
          duration_ms: asNum(stats.duration_ms) ?? 0,
        },
      }
    }
    case 'log': {
      const message = asStr(raw.message)
      if (message === '') return null
      return { type: 'log', level: asStr(raw.level, 'info'), message }
    }
    case 'clone_done': {
      const repoId = asId(raw.repo_id)
      if (repoId === null) return null
      return { type: 'clone_done', repo_id: repoId }
    }
    case 'clone_failed': {
      const repoId = asId(raw.repo_id)
      const error = asStr(raw.error)
      if (repoId === null || error === '') return null
      return { type: 'clone_failed', repo_id: repoId, error }
    }
    default:
      return null
  }
}
