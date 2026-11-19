# code-grapher REST API 契约

前端（web/）与 CLI（--server 模式）共同消费。所有响应为 JSON。

## 通用类型
- SymbolKind: `function | method | closure | struct | enum | trait | type | const | static | macro | impl | mod`
- EdgeKind: `direct_fn | dynamic_method | callback | fn_ptr | macro`
- EdgeSource: `ast | llm`
- TaskStatus: `pending | running | done | failed`

## Repos
- `GET /api/repos` → `{repos:[Repo]}`，Repo 含 `stats:{files,symbols,edges,pending_tasks}` 与 `status`
- `POST /api/repos` 两种模式，**注册即自动触发全量索引**（索引完成自动入队 LLM 理解任务）：
  - url 模式（主路径）body `{url, name?}`：校验并归一化 url（`canonical_git_url`）→ 查重（同一 canonical url 已注册返回 409 + `{error, repo}`）→ 插入 `status:'cloning'` 行并**立即返回** → 后台 `git clone` 到 `clones_root`（`CG_REPOS_DIR` 覆盖，默认 `./repos`；克隆禁用终端交互，私有仓库凭证走服务器 git 环境）→ 成功置 `ready`、WS 推 `clone_done`、启动 watcher 并自动索引；失败置 `failed`（`error` 带原因）、WS 推 `clone_failed`。克隆目录名 `净化名-fnv1a16(url)` 防穿越/防冲突。服务重启时中断的 cloning 行置 `failed`
  - path 模式（兼容）body `{path, name?, git_url?}`：注册本地已存在 checkout，upsert 后同样自动索引
- Repo 新增 `status: cloning|ready|failed` 与 `error`（failed 时的原因）
- `DELETE /api/repos/{id}` → `{ok:true}`（本期不删除 clones_root 下的克隆目录）
- `POST /api/repos/{id}/index` body `{full?:bool}` → `{ok:true}`（或 `{run_id:null,already_running:true}`）；`status=cloning` 时返回 409；进度走 WS
- `POST /api/repos/{id}/analyze` → `{enqueued:N}`（为全部函数入队 hidden_calls 任务）
- `GET /api/repos/{id}` → `{repo, last_run, stats, feature_repo_id}`
- Repo 响应均含计算字段 `feature_repo_id = sha256(canonical(git_url))`；`git_url` 缺失或非法时为 `null`（前端据此提示“该仓库未配置可用 git_url，无法使用功能评审”）

## Features（功能评审，MR 分支模型，git_url 身份）
`repo_id` 一律是 `sha256(canonical_git_url)`（64 位小写 hex），不是数据库自增 ID。每个 `repo_id` 在 feature-root 下是**一个独立 git 文档仓库**：`main` = 已合并稳定态，`mr/<功能名>` 分支 = 该功能的草稿（MR），`docs/<功能名>/` = 该 MR 的文件命名空间（任意 `.md`/`.json` 等文本树），`_changelog/` = 审批记录。MR 唯一 ID 即 `name`（中文可用；拒绝空格、`/`、`~^:?*[\\`、`..`、`.lock` 结尾、`_` 前缀保留名、控制字符，≤128 字节）。

- `GET /api/features/{repo_id}/mrs` → `{repo_id, mrs:[{name,state:'open'|'merged',tip,ahead,behind,author,updated_at,reviewer?,merge_commit?,merged_at?,changelog?}]}`（新到旧；分区不存在返回空列表）
- `GET /api/features/{repo_id}/tree?mr=NAME` → `{repo_id, mr, files:[{path,size,state:'draft'|'stable',render:'markdown'|'json'|'text'}]}`；`mr` 省略 = main；`draft` = 该文件相对 main 有差异；不存在 404
- `GET /api/features/{repo_id}/blob?path=P&mr=NAME` → `{path,size,truncated,content,render}`；内容超 512KB 截断（`truncated:true`）；不存在 404
- `GET /api/features/{repo_id}/changelog` → `{repo_id, changelog:[{file,markdown}]}`（新到旧）
- `POST /api/features/{repo_id}/draft` body `{name, files:[{path,content}]}` → `{repo_id, outcome:'draft', commit} | {outcome:'noop'}`；把 `files` 镜像到 `mr/<name>` 分支并 commit（内容不变为 noop）；`path` 必须位于 `docs/<name>/` 之下；绝不 approve
- `GET /api/features/{repo_id}/{feature_name}/diff` → `{repo_id,name,diff}`（`git diff main...mr/<name>` 的 unified diff；已合并为空串）
- `GET /api/features/{repo_id}/{feature_name}/commits` → `{repo_id,name,commits:[{hash,author,timestamp,subject}]}`（旧→新；open = `main..branch`，merged = 该 merge 带入的分支侧提交）
- `POST /api/features/{repo_id}/{feature_name}/approve` body `{reviewer, note?, expected_tip?}` → `{repo_id, ok:true, merge_commit, mr_commit, changelog}`；执行 `git merge --no-ff mr/<name>` 进 main（reviewer 为 merge 作者）+ 追加 `_changelog/YYYY-MM-DD-<name>-<hash6>.md`；`expected_tip` 与分支 tip 不符或已合并返回 409；冲突时 400（提示先刷新 draft）

写路径（draft/approve）由 cg-feature 内部 `flock(<repo_id>/.lock)` 串行化；约定**每个分区仅一个写者**（即 cg-server 单实例）。读路径只走 git refs（`ls-tree`/`show`），无需锁、永不读到半写状态。

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
- `GET /api/config` → `{db:{mode:"local"|"remote"}, opencode:{url_set:bool, concurrency:N, paused:bool}, version}`（绝不回显密钥）
- `PUT /api/config` body `{analyze_concurrency:0..16}` → `{ok:true,analyze_concurrency,paused}`（运行时调整，0=暂停消费 pending 任务；running 任务自然跑完）

## WebSocket
- `GET /ws/events`：JSON 文本帧
  - `{type:"repo_added", repo_id}`
  - `{type:"clone_done", repo_id}` / `{type:"clone_failed", repo_id, error}`
  - `{type:"index_progress", repo_id, run_id, done, total, phase}`
  - `{type:"index_done", repo_id, run_id, stats:{files_changed,symbols,edges,duration_ms}}`
  - `{type:"index_failed", repo_id, error}`
  - `{type:"task_update", repo_id, task_id, status}`
  - `{type:"analyze_enqueued", repo_id, count}`
  - `{type:"log", level, message}`

## 补充端点

- `GET /api/health` → `{status:"ok", version}`（探活；不带认证语义）
