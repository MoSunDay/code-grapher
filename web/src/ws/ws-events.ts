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
      if (repoId === null || done === null || total === null) return null
      return {
        type: 'index_progress',
        repo_id: repoId,
        run_id: runId,
        done,
        total,
        phase: asStr(raw.phase),
        current_file: asStr(raw.current_file),
      }
    }
    case 'index_done': {
      const repoId = asId(raw.repo_id)
      const runId = asId(raw.run_id)
      const stats = isRecord(raw.stats) ? raw.stats : null
      if (repoId === null || runId === null || stats === null) return null
      const symbols = asNum(stats.symbols) ?? 0
      const edges = asNum(stats.edges) ?? 0
      return {
        type: 'index_done',
        repo_id: repoId,
        run_id: runId,
        stats: {
          files_changed: asNum(stats.files_changed) ?? 0,
          symbols,
          edges,
          symbols_total: asNum(stats.symbols_total) ?? symbols,
          edges_total: asNum(stats.edges_total) ?? edges,
          duration_ms: asNum(stats.duration_ms) ?? 0,
        },
      }
    }
    case 'repo_added': {
      const repoId = asId(raw.repo_id)
      if (repoId === null) return null
      return { type: 'repo_added', repo_id: repoId }
    }
    case 'analyze_enqueued': {
      const repoId = asId(raw.repo_id)
      const count = asNum(raw.count)
      if (repoId === null || count === null) return null
      return { type: 'analyze_enqueued', repo_id: repoId, count }
    }
    case 'config_updated': {
      const analyzeConcurrency = asNum(raw.analyze_concurrency)
      if (analyzeConcurrency === null || typeof raw.paused !== 'boolean') return null
      return {
        type: 'config_updated',
        analyze_concurrency: analyzeConcurrency,
        paused: raw.paused,
      }
    }
    case 'lagged': {
      const n = asNum(raw.n)
      if (n === null) return null
      return { type: 'lagged', n }
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
    case 'index_failed': {
      const repoId = asId(raw.repo_id)
      const runId = asId(raw.run_id)
      const error = asStr(raw.error)
      if (repoId === null || error === '') return null
      return { type: 'index_failed', repo_id: repoId, run_id: runId, error }
    }
    default:
      return null
  }
}
