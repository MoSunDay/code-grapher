# opencode server HTTP 契约（已实测验证）

本地/远程 opencode server（`opencode serve`），LLM 能力完全经它获得。

## 鉴权
- 启动：`opencode serve --port <port> [--token <token>]`；token 亦可经环境变量 `OPENCODER_SERVER_TOKEN` 传递
- 客户端侧配置：`OPENCODER_SERVER_URL`（默认 `http://127.0.0.1:34096`）+ `OPENCODER_API_KEY`（Bearer token）
- 所有请求带 `Authorization: Bearer <token>`

## 会话流程
1. `POST /api/sessions`  body `{"title":"..."}` → `{"id":"<session_id>"}`
2. `POST /api/sessions/{id}/prompt`  body `{"prompt":"<完整提示词>"}` → `{"admitted_seq":N,"ok":true}`
3. 轮询 `GET /api/sessions/{id}/messages`：
   - 响应 `{id, messages:[{id, role, blocks:[{kind:"text", text}], model?, created_at}], meta:{...}}`
   - 判定完成：出现 role=assistant 且 created_at > prompt 提交时间 的消息；回复文本 = 该消息所有 kind=text blocks 的 text 拼接
4. `DELETE /api/sessions/{id}` 清理会话（可选）

## 注意
- 会话有上下文累积，分析任务应一函数（或一小批）一会话，用完即删
- 401 → {"error":"unauthorized: missing or invalid token","ok":false}
