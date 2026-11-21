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
- **failed 重试**：对 `failed` 仓库重发同一 URL = 重置为 `cloning` 并重新克隆（响应含 `retried:true`）；`cloning`/`ready` 状态仍返回 409
- **克隆超时**：`CG_CLONE_TIMEOUT` 环境变量（秒，默认 `1800`，`0` 或非法值 = 不限）；超时置 `failed`（error 带原因）并清理半克隆目录
- **单实例假设（Repos）**：多实例共享同一 DB 时，B 实例启动会把 A 实例进行中的 `cloning` 行误置 `failed`（与 Features 段"每分区单写者"声明一致，生产请保持 cg-server 单实例）
- `DELETE /api/repos/{id}` → `{ok:true}`（`cloning` 克隆中或正在索引时返回 409；不存在返回 404）。删除成功会停止该仓库的 watcher（本进程内的监听句柄一并移除）；本期仍不删除 `clones_root` 下的克隆目录
- `POST /api/repos/{id}/index` body `{full?:bool}` → `{ok:true}`（已有索引在跑时为 `{run_id:null,already_running:true}`，此处 `null` 仅表示未新建 run）；`status=cloning` 时返回 409；进度走 WS
- **索引单飞**：同一仓库同一时刻只允许一个索引任务（API 与 watcher 共享 per-repo 锁）。API 冲突时直接返回 `already_running`；watcher 事件不丢弃，排队等当前 run 结束后以 `trigger=watch` 执行，保证变更不丢、图谱不并发写
- `POST /api/repos/{id}/analyze` → `{enqueued:N}`（为全部函数入队 hidden_calls 任务）
- `GET /api/repos/{id}` → `{repo, last_run, stats, feature_repo_id}`
- Repo 响应均含计算字段 `feature_repo_id = sha256(canonical(git_url))`；`git_url` 缺失或非法时为 `null`（前端据此提示“该仓库未配置 Git URL，仓库理解不可用”并引导补填 Git URL 或改选 URL 注册的仓库）

## Features（仓库理解，MR 分支模型，git_url 身份）
`repo_id` 一律是 `sha256(canonical_git_url)`（64 位小写 hex），不是数据库自增 ID。每个 `repo_id` 在 feature-root 下是**一个独立 git 文档仓库**：`main` = 已合并稳定态，`mr/<功能名>` 分支 = 该功能的草稿（MR），`docs/<功能名>/` = 该 MR 的文件命名空间（任意 `.md`/`.json` 等文本树），`_changelog/` = 审批记录。MR 唯一 ID 即 `name`（中文可用；拒绝空格、`/`、`~^:?*[\\`、`..`、`.lock` 结尾、`_` 前缀保留名、控制字符，≤128 字节）。

- `GET /api/features/{repo_id}/mrs` → `{repo_id, mrs:[{name,state:'open'|'merged',tip,ahead,behind,author,updated_at,reviewer?,merge_commit?,merged_at?,changelog?}]}`（新到旧；分区不存在返回空列表）
- `GET /api/features/{repo_id}/tree?mr=NAME` → `{repo_id, mr, files:[{path,size,state:'draft'|'stable',render:'markdown'|'json'|'text'}]}`；`mr` 省略 = main；`draft` = 该文件相对 main 有差异；`mr` 不存在 404；分区不存在时自动初始化空存储并返回空树 200
- `GET /api/features/{repo_id}/blob?path=P&mr=NAME` → `{path,size,truncated,content,render}`；内容超 512KB 截断（`truncated:true`）；文件不存在 404；分区不存在时自动初始化空存储（同 tree）
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
- 无对外查询/管理端点：仓库理解任务（hidden_calls）由索引完成自动入队、调度器后台执行；LLM 隐藏调用边（source=llm，带 evidence/confidence）直接落库并入图谱

## Stats（Dashboard）
- `GET /api/repos/{id}/stats` → `{totals:{files,symbols,edges,functions}, symbols_by_kind:{...}, edges_by_kind:{...}, edges_by_source:{ast,llm}, task_status:{pending,running,done,failed}, recent_runs:[IndexRun]}`
- `totals` 是当前图谱**去重后**的计数（直接来自 files/symbols/call_edges 表）；Dashboard 顶部统计卡展示此口径
- `recent_runs` 为最近 10 次索引（新到旧）：`symbols`/`edges` 是该次**解析阶段计数**（各文件解析结果累加、落库去重前），`files_changed` 为本次变更文件数；Recent Runs 表格展示此口径，不要与 `totals` 卡混淆

## Config（Settings 页）
- `GET /api/config` → `{db:{mode:"local"|"remote"}, opencode:{url_set:bool, concurrency:N, paused:bool}, version}`（绝不回显密钥）
- `PUT /api/config` body `{analyze_concurrency:0..16}` → `{ok:true,analyze_concurrency,paused}`（运行时调整，0=暂停消费 pending 任务；running 任务自然跑完）

## WebSocket
- `GET /ws/events`：JSON 文本帧
  - `{type:"repo_added", repo_id}`
  - `{type:"clone_done", repo_id}` / `{type:"clone_failed", repo_id, error}`
  - `{type:"index_progress", repo_id, run_id, done, total, phase, current_file}`：`run_id` 为本次 index run 的真实 id（`index_runs.id`）；服务端新版本始终非 null，仅旧版服务端可能为 null（前端保留兼容）。`total` 是本次**待解析的变更文件数**（不是全仓库扫描数）；解析开始前先发一帧 `done=0,total=0`（前端显示“准备中”），末帧满足 `done==total`；`current_file` 为正在解析的文件（完成时为空串）。前端只接受与当前 active run 一致的进度帧（并记住最近结束的 run，丢弃其迟到帧），其他 run 的进度直接忽略；`run_id` 为 null 的旧版帧仍然接受
  - `{type:"index_done", repo_id, run_id, stats:{files_changed,symbols,edges,symbols_total,edges_total,duration_ms}}`：`symbols`/`edges` 是本次**解析阶段计数**（落库去重前）；`symbols_total`/`edges_total` 是完成时图谱内**去重后的当前总数**（旧服务端缺省时前端回退为 symbols/edges；前端文案区分“本次解析”与“图内共”）；watch 触发的 run 同样携带这些字段
  - `{type:"index_failed", repo_id, run_id, error}`：索引任务失败，`error` 为格式化错误链；`run_id` 为失败的那次 run，若失败发生在创建 run 之前（如 watcher 初始化失败）则为 null；本次不会再发 `index_done`，前端应停止进度并提示可重试（再次 `POST /api/repos/{id}/index`）；失败前已写入的解析结果不回滚
  - `{type:"config_updated", analyze_concurrency, paused}`：`PUT /api/config` 成功后广播；`paused` 等价于 `analyze_concurrency==0`（暂停消费 pending 任务，running 任务自然跑完）
  - `{type:"analyze_enqueued", repo_id, count}`
  - `{type:"lagged", n}`：WS broadcast 通道积压，`n` 为被丢弃的消息条数（慢消费者）；前端据此提示日志可能不完整
  - `{type:"log", level, message}`

## 补充端点

- `GET /api/health` → `{status:"ok", version}`（探活；不带认证语义）
