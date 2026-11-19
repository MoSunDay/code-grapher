import { Alert, Empty, Spin, Typography } from 'antd'
import { useQuery } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { getBlob } from '../../api/features'
import { QueryErrorAlert } from '../../components/QueryErrorAlert'
import { formatSize } from '../../lib/format'
import { MarkdownView } from './MarkdownView'

/** Right pane: render one blob as markdown / pretty JSON / plain text. */
export function ContentView({
  featureRepoId,
  mr,
  path,
}: {
  featureRepoId: string
  mr: string | null
  path: string | null
}): ReactNode {
  const blob = useQuery({
    queryKey: ['feature-blob', featureRepoId, mr, path],
    queryFn: () => getBlob(featureRepoId, path ?? '', mr ?? undefined),
    enabled: path !== null,
  })

  if (path === null) {
    return <Empty description="选择左侧文件查看内容" style={{ marginTop: 80 }} />
  }
  if (blob.isLoading) {
    return (
      <div style={{ padding: 48, textAlign: 'center' }}>
        <Spin />
      </div>
    )
  }
  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', minWidth: 0 }}>
      <QueryErrorAlert error={blob.error} context="读取文件内容" />
      <Typography.Text
        type="secondary"
        style={{ fontSize: 12, marginBottom: 8, flex: 'none' }}
      >
        {path} · {formatSize(blob.data?.size ?? 0)}
        {blob.data?.truncated === true ? ' · 已截断（超过 512KB）' : ''}
      </Typography.Text>
      <div style={{ flex: 1, minWidth: 0, overflow: 'auto' }}>{render(blob.data?.content, blob.data?.render ?? 'text')}</div>
    </div>
  )
}

function render(content: string | undefined, hint: string): ReactNode {
  if (content === undefined) return null
  if (hint === 'markdown') return <MarkdownView markdown={content} />
  if (hint === 'json') return <JsonView content={content} />
  return (
    <pre style={{ margin: 0, whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>{content}</pre>
  )
}

function JsonView({ content }: { content: string }): ReactNode {
  try {
    const parsed: unknown = JSON.parse(content)
    return (
      <pre style={{ margin: 0, whiteSpace: 'pre-wrap' }}>
        {JSON.stringify(parsed, null, 2)}
      </pre>
    )
  } catch {
    return (
      <>
        <Alert type="warning" message="JSON 解析失败，按纯文本显示" style={{ marginBottom: 8 }} />
        <pre style={{ margin: 0, whiteSpace: 'pre-wrap' }}>{content}</pre>
      </>
    )
  }
}
