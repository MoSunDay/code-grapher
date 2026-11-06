import { RedoOutlined } from '@ant-design/icons'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { App, Button, Empty, Popconfirm, Select, Space, Table, Tabs, Tag, Tooltip, Typography } from 'antd'
import type { ColumnsType } from 'antd/es/table'
import { useState, type ReactNode } from 'react'
import { Link } from 'react-router-dom'
import type { Id, Task, TaskStatus } from '../api/types'
import { retryTask } from '../api/tasks'
import { KindTag, TaskStatusBadge } from '../components/Badges'
import { QueryErrorAlert } from '../components/QueryErrorAlert'
import { useRepo } from '../repo/use-repo'
import { useTasks } from '../hooks/queries'
import { fmtTime } from '../lib/format'
import { UnresolvedEdges } from './tasks/UnresolvedEdges'

const STATUS_OPTIONS: Array<{ value: TaskStatus; label: string }> = [
  { value: 'pending', label: 'pending / 待处理' },
  { value: 'running', label: 'running / 运行中' },
  { value: 'done', label: 'done / 完成' },
  { value: 'failed', label: 'failed / 失败' },
]

export default function TasksPage(): ReactNode {
  const { repoId } = useRepo()
  const { message } = App.useApp()
  const queryClient = useQueryClient()
  const [status, setStatus] = useState<TaskStatus | undefined>(undefined)
  const query = useTasks(repoId, status)
  const tasks = query.data?.tasks ?? []

  const retryMutation = useMutation({
    mutationFn: (taskId: Id) => retryTask(taskId),
    onSuccess: () => {
      message.success('已重新入队该任务')
      void queryClient.invalidateQueries({ queryKey: ['repos'] })
    },
  })

  const columns: ColumnsType<Task> = [
    {
      title: 'ID',
      dataIndex: 'id',
      width: 80,
      render: (v: Id) => <Typography.Text code>{String(v)}</Typography.Text>,
    },
    { title: '类型', dataIndex: 'type', width: 130, render: (v: string) => <Tag>{v}</Tag> },
    {
      title: '状态',
      dataIndex: 'status',
      width: 100,
      render: (v: TaskStatus) => <TaskStatusBadge status={v} />,
    },
    { title: '尝试', dataIndex: 'attempts', width: 70, align: 'right' },
    {
      title: '目标符号',
      key: 'target',
      ellipsis: true,
      render: (_, record) =>
        record.target_symbol !== null && record.target_symbol !== undefined ? (
          <Link to={`/functions?symbol=${String(record.target_symbol.id)}`}>
            <Space size={4}>
              <Typography.Text>{record.target_symbol.name}</Typography.Text>
              <KindTag kind={record.target_symbol.kind} />
            </Space>
          </Link>
        ) : (
          '-'
        ),
    },
    {
      title: '错误',
      dataIndex: 'error',
      ellipsis: true,
      render: (v?: string | null) =>
        v ? (
          <Tooltip title={v}>
            <Typography.Text type="danger" ellipsis style={{ maxWidth: 260 }}>
              {v}
            </Typography.Text>
          </Tooltip>
        ) : (
          '-'
        ),
    },
    { title: '更新时间', dataIndex: 'updated_at', width: 170, render: (v) => fmtTime(v) },
    {
      title: '操作',
      key: 'actions',
      width: 90,
      render: (_, record) =>
        record.status === 'failed' ? (
          <Popconfirm title="重试该任务？" onConfirm={() => retryMutation.mutate(record.id)}>
            <Button size="small" icon={<RedoOutlined />} loading={retryMutation.isPending}>
              重试
            </Button>
          </Popconfirm>
        ) : null,
    },
  ]

  return (
    <div>
      <QueryErrorAlert error={query.error} context="加载任务列表" />
      <div className="page-head">
        <Typography.Title level={4} style={{ margin: 0 }}>
          Tasks
        </Typography.Title>
        <Select<TaskStatus>
          allowClear
          placeholder="状态（全部）"
          style={{ minWidth: 160 }}
          value={status}
          options={STATUS_OPTIONS}
          onChange={(value) => setStatus(value)}
        />
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          LLM 分析任务与人审队列
        </Typography.Text>
      </div>
      <Tabs
        defaultActiveKey="tasks"
        items={[
          {
            key: 'tasks',
            label: `分析任务（${tasks.length}）`,
            children: (
              <Table
                rowKey="id"
                size="small"
                columns={columns}
                dataSource={tasks}
                loading={query.isLoading}
                pagination={{
                  pageSize: 20,
                  showSizeChanger: false,
                  showTotal: (total) => `共 ${total} 项`,
                }}
                locale={{ emptyText: <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无任务" /> }}
              />
            ),
          },
          {
            key: 'review',
            label: '人审边',
            children: repoId !== null ? <UnresolvedEdges repoId={repoId} /> : null,
          },
        ]}
      />
    </div>
  )
}
