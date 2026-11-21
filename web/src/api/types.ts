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
export type EdgeDirection = 'both' | 'callers' | 'callees'

export interface RepoStats {
  files: number
  symbols: number
  edges: number
  pending_tasks: number
}

export type RepoStatus = 'cloning' | 'ready' | 'failed'

export interface Repo {
  id: Id
  name: string
  path: string
  git_url?: string | null
  /** sha256(canonical git_url)；缺失/非法 git_url 时为 null，仓库理解不可用 */
  feature_repo_id?: string | null
  /** cloning = 后台克隆中；failed 见 error */
  status?: RepoStatus
  error?: string | null
  stats?: RepoStats
}
export interface RepoList {
  repos: Repo[]
}

/** url 模式：服务端后台 git clone + 自动索引；path 模式：注册本地已存在的 checkout */
export type CreateRepoInput =
  | { url: string; name?: string }
  | { path: string; name?: string; git_url?: string }
export interface IndexTriggerResult {
  ok?: boolean
  run_id?: Id | null
  already_running?: boolean
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
  opencode: { url_set: boolean; concurrency: number; paused: boolean }
  version: string
}

export interface UpdateConfigResult {
  ok: boolean
  analyze_concurrency: number
  paused: boolean
}

export interface IndexProgressEvent {
  type: 'index_progress'
  repo_id: Id
  run_id: Id | null
  done: number
  total: number
  phase: string
  /** 正在解析的文件（相对路径）；准备/完成时为空串 */
  current_file: string
}
export interface IndexDoneEvent {
  type: 'index_done'
  repo_id: Id
  run_id: Id
  stats: {
    files_changed: number
    /** 解析阶段产生的符号数（去重前） */
    symbols: number
    /** 解析阶段产生的边数（去重前） */
    edges: number
    /** 图内去重后的符号总数；缺省时前端回退到 symbols */
    symbols_total?: number
    /** 图内去重后的边总数；缺省时前端回退到 edges */
    edges_total?: number
    duration_ms: number
  }
}
export interface LogEvent {
  type: 'log'
  level: string
  message: string
}
export interface CloneDoneEvent {
  type: 'clone_done'
  repo_id: Id
}
export interface RepoAddedEvent {
  type: 'repo_added'
  repo_id: Id
}
export interface CloneFailedEvent {
  type: 'clone_failed'
  repo_id: Id
  error: string
}
export interface IndexFailedEvent {
  type: 'index_failed'
  repo_id: Id
  run_id?: Id | null
  error: string
}
export interface AnalyzeEnqueuedEvent {
  type: 'analyze_enqueued'
  repo_id: Id
  count: number
}
export interface ConfigUpdatedEvent {
  type: 'config_updated'
  analyze_concurrency: number
  paused: boolean
}
export interface LaggedEvent {
  type: 'lagged'
  /** WS broadcast 丢弃的消息数 */
  n: number
}
export type WsEvent =
  | IndexProgressEvent
  | IndexDoneEvent
  | IndexFailedEvent
  | LogEvent
  | CloneDoneEvent
  | CloneFailedEvent
  | RepoAddedEvent
  | AnalyzeEnqueuedEvent
  | ConfigUpdatedEvent
  | LaggedEvent

export function toId(value: string): Id {
  const n = Number(value)
  return Number.isFinite(n) && String(n) === value ? n : value
}

export type MrState = 'open' | 'merged'

export interface MrSummary {
  name: string
  state: MrState
  tip: string
  ahead: number
  behind: number
  author: string
  updated_at: number
  reviewer: string | null
  merge_commit: string | null
  merged_at: number | null
  changelog: string | null
}

export interface MrList {
  repo_id: string
  mrs: MrSummary[]
}

export type FileBadge = 'draft' | 'stable'
export type RenderHint = 'markdown' | 'json' | 'text'

export interface TreeFile {
  path: string
  size: number
  state: FileBadge
  render: RenderHint
}

export interface FeatureTree {
  repo_id: string
  mr: string | null
  files: TreeFile[]
}

export interface BlobContent {
  path: string
  size: number
  truncated: boolean
  content: string
  render: RenderHint
}

export interface MrCommit {
  hash: string
  author: string
  timestamp: number
  subject: string
}

export interface MrCommits {
  repo_id: string
  name: string
  commits: MrCommit[]
}

export interface FeatureDiff {
  repo_id: string
  name: string
  diff: string
}

export interface FeatureDraftResult {
  repo_id: string
  outcome: 'draft' | 'noop'
  commit?: string
}

export interface FeatureApproveResult {
  repo_id: string
  ok: boolean
  merge_commit: string
  mr_commit: string
  changelog: string
}

export interface ChangelogEntry {
  file: string
  markdown: string
}

export interface ChangelogList {
  repo_id: string
  changelog: ChangelogEntry[]
}
