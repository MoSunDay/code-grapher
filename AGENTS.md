# AGENTS.md — code-grapher 仓库须知

仓库代码图谱系统：Rust 后端（axum + tree-sitter + libsql/Turso）+ React/antd 前端 + LLM（opencode server）动态调用补齐。

## 常用命令
- 构建：`cargo build --workspace`
- 测试：`cargo test --workspace`
- 单测某 crate：`cargo test -p cg-core`
- 前端：`cd web && npm run dev / build / lint`
- 本地服务：`cargo run -p cg-cli -- serve --port 7800`（默认本地 libsql 文件 `./code-grapher.db`）

## 布局
- `crates/cg-core`：领域模型 + tree-sitter 解析（语言注册表，Rust 优先）
- `crates/cg-db`：libsql/Turso 访问层，migrations 在根 `migrations/`（由 cg-db include_str 嵌入）
- `crates/cg-indexer`：仓库扫描、增量索引（fnv1a hash + mtime）、文件监听
- `crates/cg-analyzer`：LLM prompt 构造、opencode server 客户端、任务调度（信号量 worker 池）
- `crates/cg-server`：axum REST + WS 守护服务
- `crates/cg-cli`：clap CLI（本地嵌入 / --server 远程两模式）
- `web/`：Vite + React + TS strict + antd5 + TanStack Query + G6

## 硬性规范
- 每文件 ≤400 行（新增）/ ≤800 行（迭代中），超限按职责拆分
- 纯函数式：禁 class/OOP；状态经参数与返回值传递
- 凭证只走环境变量：`TURSO_DATABASE_URL`、`TURSO_AUTH_TOKEN`、`OPENCODER_SERVER_URL`、`OPENCODER_API_KEY`；代码零硬编码
- opencode server 契约见 `docs/opencode-server.md`；REST API 契约见 `docs/api.md`
- 解析器测试用 `crates/cg-core/tests/fixtures/` 快照；analyzer 测试用 mock server
