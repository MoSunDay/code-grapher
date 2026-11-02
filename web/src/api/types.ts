export type Id = number | string

export type SymbolKind =
  | 'function'
  | 'method'
  | 'closure'
  | 'struct'
  | 'enum'
  | 'trait'
  | 'type'
  | 'const'
  | 'static'
  | 'macro'
  | 'impl'
  | 'mod'

export type EdgeKind = 'direct_fn' | 'dynamic_method' | 'callback' | 'fn_ptr' | 'macro'
export type EdgeSource = 'ast' | 'llm'
export type TaskStatus = 'pending' | 'running' | 'done' | 'failed'
export type EdgeDirection = 'both' | 'callers' | 'callees'

export interface RepoStats {
  files: number
  symbols: number
  edges: number
  pending_tasks: number
}

export interface Repo {
  id: Id
  name: string
  path: string
  git_url?: string | null
  stats?: RepoStats
}
export interface RepoList {
  repos: Repo[]
}

export interface CreateRepoInput {
  path: string
  name?: string
  git_url?: string
}
export interface IndexRunRef {
  run_id: Id
}
export interface AnalyzeResult {
  enqueued: number
}

export interface FileMeta {
  id: Id
  path: string
  lang: string
  loc: number
  mtime: string | number
  parsed_at?: string | number | null
  symbol_count: number
}
export interface FileList {
  files: FileMeta[]
}
export interface FileContent {
  file: FileMeta
  content: string
}

export interface SymbolFileRef {
  id: Id
  path: string
}
export interface Symbol {
  id: Id
  kind: SymbolKind
  name: string
  qual_path: string
  file: SymbolFileRef
  start_line: number
  end_line: number
  is_pub: boolean
}
export interface SymbolList {
  total: number
  symbols: Symbol[]
}
export interface SymbolQueryParams {
  kind?: SymbolKind
  q?: string
  file_id?: Id
  limit?: number
  offset?: number
}

export interface EdgeSymbolRef {
  id: Id
  name: string
  qual_path: string
  kind: SymbolKind
  file?: { path: string } | null
  start_line?: number
}
export interface EdgeView {
  id: Id
  kind: EdgeKind
  source: EdgeSource
  confidence: number
  callee_name: string
  callee?: EdgeSymbolRef | null
  caller?: EdgeSymbolRef | null
}
export interface SymbolDetail {
  symbol: Symbol
  callers: EdgeView[]
  callees: EdgeView[]
}

export interface GraphNode {
  id: Id
  name: string
  kind: string
  qual_path: string
}
export interface GraphEdge {
  id: Id
  source_id: Id
  target_id?: Id | null
  callee_name: string
  kind: EdgeKind
  source: EdgeSource
  confidence: number
}
export interface CallGraph {
  nodes: GraphNode[]
  edges: GraphEdge[]
}

export interface TaskSymbolRef {
  id: Id
  name: string
  kind: SymbolKind
  qual_path: string
}
export interface Task {
  id: Id
  type: string
  status: TaskStatus
  attempts: number
  error?: string | null
  created_at: string | number
  updated_at: string | number
  target_symbol?: TaskSymbolRef | null
}
export interface TaskList {
  tasks: Task[]
}
export interface UnresolvedEdgeList {
  edges: EdgeView[]
}

export interface IndexRun {
  id: Id
  status?: string
  full?: boolean
  started_at?: string | number | null
  finished_at?: string | number | null
  files_changed?: number
  symbols?: number
  edges?: number
  duration_ms?: number | null
  error?: string | null
}

export interface RepoStatsPayload {
  totals: { files: number; symbols: number; edges: number; functions: number }
  symbols_by_kind: Record<string, number>
  edges_by_kind: Record<string, number>
  edges_by_source: { ast: number; llm: number }
  task_status: { pending: number; running: number; done: number; failed: number }
  recent_runs: IndexRun[]
}

export interface ServerConfig {
  db: { mode: 'local' | 'remote' }
  opencode: { url_set: boolean; concurrency: number }
  version: string
}

export interface IndexProgressEvent {
  type: 'index_progress'
  repo_id: Id
  run_id: Id
  done: number
  total: number
  phase: string
}
export interface IndexDoneEvent {
  type: 'index_done'
  repo_id: Id
  run_id: Id
  stats: { files_changed: number; symbols: number; edges: number; duration_ms: number }
}
export interface TaskUpdateEvent {
  type: 'task_update'
  repo_id: Id
  task_id: Id
  status: TaskStatus
}
export interface LogEvent {
  type: 'log'
  level: string
  message: string
}
export type WsEvent = IndexProgressEvent | IndexDoneEvent | TaskUpdateEvent | LogEvent

export function toId(value: string): Id {
  const n = Number(value)
  return Number.isFinite(n) && String(n) === value ? n : value
}
