import { Badge, Table } from 'antd'
import type { ColumnsType } from 'antd/es/table'
import type { ReactNode } from 'react'
import type { IndexRun } from '../../api/types'
import { fmtDuration, fmtNumber, fmtTime } from '../../lib/format'

const STATUS_META: Record<
  string,
  { badge: 'success' | 'processing' | 'error' | 'default'; text: string }
> = {
  running: { badge: 'processing', text: '进行中' },
  done: { badge: 'success', text: '完成' },
  failed: { badge: 'error', text: '失败' },
}

export function RecentRunsTable({ runs }: { runs: IndexRun[] }): ReactNode {
  const columns: ColumnsType<IndexRun> = [
    { title: 'Run', dataIndex: 'id', width: 80, render: (v: string | number) => String(v) },
    {
      title: '状态',
      dataIndex: 'status',
      width: 100,
      render: (status?: string) => {
        const meta = STATUS_META[status ?? ''] ?? { badge: 'default' as const, text: status ?? '-' }
        return <Badge status={meta.badge} text={meta.text} />
      },
    },
    { title: '开始时间', dataIndex: 'started_at', width: 170, render: (v) => fmtTime(v) },
    {
      title: '用时',
      dataIndex: 'duration_ms',
      width: 90,
      render: (v?: number | null) => fmtDuration(v),
    },
    {
      title: '文件变更',
      dataIndex: 'files_changed',
      width: 100,
      align: 'right',
      render: (v?: number) => fmtNumber(v),
    },
    {
      title: '符号',
      dataIndex: 'symbols',
      width: 100,
      align: 'right',
      render: (v?: number) => fmtNumber(v),
    },
    {
      title: '边',
      dataIndex: 'edges',
      width: 100,
      align: 'right',
      render: (v?: number) => fmtNumber(v),
    },
  ]
  return (
    <Table
      rowKey="id"
      size="small"
      columns={columns}
      dataSource={runs}
      pagination={false}
      locale={{ emptyText: '暂无索引运行记录' }}
    />
  )
}
