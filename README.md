# code-grapher

仓库代码图谱系统：tree-sitter 静态解析 + LLM 动态补齐 + libsql/Turso 存储 + Web 可视化。

## 架构

```
crates/
  cg-core      领域模型 + tree-sitter 解析（Rust 优先；符号/调用边提取）
  cg-db        libsql/Turso 访问层（migrations 嵌入）
  cg-indexer   仓库扫描、增量索引（fnv1a + mtime）、文件监听
  cg-analyzer  LLM prompt 构造、opencode server 客户端、任务调度
  cg-server    axum REST + WebSocket 守护服务（默认 :7800）
  cg-cli       clap CLI（本地嵌入 / --server 远程两模式）
web/           Vite + React 18 + TS strict + antd5 + TanStack Query + G6
```

## 快速开始

```bash
cargo build --workspace && cargo test --workspace

# 本地模式（默认 ./code-grapher.db）
code-grapher repo add --name demo /path/to/repo
code-grapher index            # 增量索引；--full 全量重扫
code-grapher analyze --limit 20   # LLM 补齐隐藏调用（需 OPENCODER_* 环境变量）
code-grapher query symbols      # 查询图谱（files | symbols | calls）

# 守护服务模式（REST + WS + 自动调度 + 文件监听）
code-grapher serve --port 7800
cd web && npm run dev         # 前端 dev proxy -> 127.0.0.1:7800
```

## 环境变量（零硬编码凭证）

| 变量 | 说明 |
|---|---|
| `TURSO_DATABASE_URL` | `file:...` 本地库或 `libsql://...` 远程 Turso；缺省 `./code-grapher.db` |
| `TURSO_AUTH_TOKEN` | 远程库令牌 |
| `OPENCODER_SERVER_URL` | opencode server 地址（默认 `http://127.0.0.1:34096`） |
| `OPENCODER_API_KEY` | opencode server Bearer 令牌 |
| `CG_ANALYZE_CONCURRENCY` | 分析并发（1-16，默认 2） |
| `CG_ANALYZE_TIMEOUT_SECS` | 单任务 LLM 等待上限（10-3600，默认 180） |

## 调用边模型

- AST 边（`source=ast`）：direct_fn / dynamic_method / macro / callback-arg / fn_ptr；名字唯一可解析的自动绑定 `callee_id`
- LLM 边（`source=llm`）：LLM 提出的隐藏调用；通过唯一名校验的直接绑定，其余进入人审队列（`/api/repos/{id}/edges/unresolved`，confirm/dismiss）

## 契约文档

- REST API：`docs/api.md`；WebSocket 事件与前端页面一一对应
- opencode server：`docs/opencode-server.md`

## 测试

`cargo test --workspace`：解析快照、db roundtrip、索引/增量、mock opencode 端到端。
前端：`cd web && npm run lint && npm run build`。
