import { Progress, Typography } from 'antd'
import type { ReactNode } from 'react'
import type { Id } from '../../api/types'

export interface IndexProgressState {
  runId: Id | null
  done: number
  total: number
  phase: string
}

interface Props {
  progress: IndexProgressState | null
  logs: string[]
}

/** 运行 id 可能很长（字符串 id），只展示尾部 6 位保持标签紧凑 */
function shortRunId(id: Id): string {
  const text = String(id)
  return text.length > 6 ? `…${text.slice(-6)}` : text
}

export function IndexProgressPanel({ progress, logs }: Props): ReactNode {
  if (progress === null && logs.length === 0) return null
  const preparing = progress !== null && progress.total === 0
  const percent =
    progress !== null && progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : 0
  const progressLabel =
    progress === null
      ? ''
      : preparing
        ? '索引进度 · 准备中'
        : `索引进度 · ${progress.phase === '' ? '解析' : progress.phase} · ${progress.done}/${progress.total}`
  const runLabel =
    progress !== null && progress.runId !== null ? ` · run #${shortRunId(progress.runId)}` : ''
  return (
    <div style={{ marginTop: 12 }}>
      {progress !== null && (
        <>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            {`${progressLabel}${runLabel}`}
          </Typography.Text>
          <Progress percent={percent} size="small" status={preparing ? 'normal' : 'active'} />
        </>
      )}
      {logs.length > 0 && <pre className="log-tail">{logs.slice(-8).join('\n')}</pre>}
    </div>
  )
}
