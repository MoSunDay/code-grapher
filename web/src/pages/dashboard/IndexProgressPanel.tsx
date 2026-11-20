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

export function IndexProgressPanel({ progress, logs }: Props): ReactNode {
  if (progress === null && logs.length === 0) return null
  const preparing = progress !== null && progress.total === 0
  const percent =
    progress !== null && progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : 0
  return (
    <div style={{ marginTop: 12 }}>
      {progress !== null && (
        <>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            {preparing
              ? '索引进度 · 准备中'
              : `索引进度 · ${progress.phase === '' ? '解析' : progress.phase} · ${progress.done}/${progress.total}`}
          </Typography.Text>
          <Progress percent={percent} size="small" status={preparing ? 'normal' : 'active'} />
        </>
      )}
      {logs.length > 0 && <pre className="log-tail">{logs.slice(-8).join('\n')}</pre>}
    </div>
  )
}
