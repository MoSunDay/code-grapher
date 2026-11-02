import { Progress, Typography } from 'antd'
import type { ReactNode } from 'react'

export interface IndexProgressState {
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
  const percent =
    progress !== null && progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : 0
  return (
    <div style={{ marginTop: 12 }}>
      {progress !== null && (
        <>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            索引进度 · {progress.phase === '' ? '…' : progress.phase} · {progress.done}/
            {progress.total > 0 ? progress.total : '?'}
          </Typography.Text>
          <Progress percent={percent} size="small" status="active" />
        </>
      )}
      {logs.length > 0 && <pre className="log-tail">{logs.slice(-8).join('\n')}</pre>}
    </div>
  )
}
