# code-grapher REST API 契约

前端（web/）与 CLI（--server 模式）共同消费。所有响应为 JSON。

## 通用类型
- SymbolKind: `function | method | closure | struct | enum | trait | type | const | static | macro | impl | mod`
- EdgeKind: `direct_fn | dynamic_method | callback | fn_ptr | macro`
- EdgeSource: `ast | llm`
- TaskStatus: `pending | running | done | failed`

## Repos
- `GET /api/repos` → `{repos:[Repo]}`，Repo 含 `stats:{files,symbols,edges,pending_tasks}`
- `POST /api/repos` body `{path, name?, git_url?}` → `{repo}`
- `DELETE /api/repos/{id}` → `{ok:true}`
- `POST /api/repos/{id}/index` body `{full?:bool}` → `{run_id}`；进度走 WS
- `POST /api/repos/{id}/analyze` → `{enqueued:N}`（为全部函数入队 hidden_calls 任务）
- `GET /api/repos/{id}/status` → `{repo, last_run, stats, feature_repo_id}`
- Repo 响应均含计算字段 `feature_repo_id = sha256(canonical(git_url))`；`git_url` 缺失或非法时为 `null`（前端据此提示“该仓库未配置可用 git_url，无法使用功能评审”）

## Features（功能评审，git_url 身份）
`repo_id` 一律是 `sha256(canonical_git_url)`（64 位小写 hex），不是数据库自增 ID。功能唯一 ID 即 `name`，`name` 直接作为目录名（支持中文/空格，拒绝 `/`、`..`、控制字符、`_` 前缀保留名）。生命周期只由 git 表达：`feature.json` 有未提交变更 = draft，approve 产生的 commit = stable。

- `GET /api/features/{repo_id}` → `{repo_id, features:[{name,state:'draft'|'stable',hash}]}`（空仓库返回空列表）
- `POST /api/features/{repo_id}` body 为 feature JSON（含 `id` 字段会被拒绝）→ `{repo_id, outcome:'draft'|'noop', rel_path?}`；只刷新 draft，绝不 commit
- `GET /api/features/{repo_id}/{feature_name}` → `{repo_id,name,state,hash,feature}`；不存在返回 404
- `GET /api/features/{repo_id}/{feature_name}/diff` → `{repo_id,name,diff}`（`git diff HEAD` 的 unified diff，新文件对比 /dev/null；稳定态 diff 为空串）
- `POST /api/features/{repo_id}/{feature_name}/approve` body `{reviewer, note?, expected_hash?}` → `{repo_id, ok:true, commit, changelog}`；`expected_hash` 与当前 feature.json sha256 不符时拒绝；仅提交该 feature.json 与对应 changelog 文件，其他脏文件不入 commit
- `GET /api/features/{repo_id}/changelog` → `{repo_id, changelog:[{file,markdown}]}`（新到旧）

## Files
- `GET /api/repos/{id}/files` → `{files:[{id,path,lang,loc,mtime,parsed_at,symbol_count}]}`
- `GET /api/repos/{id}/file?path=<rel>` → `{file:{...}, content:"..."}`

## Symbols
- `GET /api/repos/{id}/symbols?kind=&q=&file_id=&limit=50&offset=0` → `{total, symbols:[{id,kind,name,qual_path,file:{id,path},start_line,end_line,is_pub}]}`
- `GET /api/symbols/{id}` → `{symbol, callers:[EdgeView], callees:[EdgeView]}`，EdgeView = `{id, kind, source, confidence, callee_name, callee?:{id,name,qual_path,kind,file:{path},start_line}, caller?:{...}}`

## Graph
- `GET /api/symbols/{id}/graph?depth=2&direction=both` → `{nodes:[{id,name,kind,qual_path}], edges:[{id,source_id,target_id?,callee_name,kind,source,confidence}]}`（target 为空表示未解析边，前端画虚线）

## Tasks
- `GET /api/repos/{id}/tasks?status=&limit=100` → `{tasks:[{id,type,status,attempts,error,created_at,updated_at,target_symbol:{id,name,kind,qual_path}}]}`
- `POST /api/tasks/{id}/retry` → `{ok:true}`
- `GET /api/repos/{id}/edges/unresolved` → `{edges:[EdgeView]}`（LLM 提出但未通过校验的边，供人审）
- `POST /api/edges/{id}/confirm` → `{ok:true}`（人工确认，confidence=1.0）
- `POST /api/edges/{id}/dismiss` → `{ok:true}`（丢弃）

## Stats（Dashboard）
- `GET /api/repos/{id}/stats` → `{totals:{files,symbols,edges,functions}, symbols_by_kind:{...}, edges_by_kind:{...}, edges_by_source:{ast,llm}, task_status:{pending,running,done,failed}, recent_runs:[IndexRun]}`

## Config（Settings 页）
- `GET /api/config` → `{db:{mode:"local"|"remote"}, opencode:{url_set:bool, concurrency:N}, version}`（绝不回显密钥）

## WebSocket
- `GET /ws/events`：JSON 文本帧
  - `{type:"index_progress", repo_id, run_id, done, total, phase}`
  - `{type:"index_done", repo_id, run_id, stats:{files_changed,symbols,edges,duration_ms}}`
  - `{type:"task_update", repo_id, task_id, status}`
  - `{type:"log", level, message}`

## 补充端点

- `GET /api/health` → `{status:"ok", version}`（探活；不带认证语义）
