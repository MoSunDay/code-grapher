import { PlayCircleOutlined, ReloadOutlined, ThunderboltOutlined } from '@ant-design/icons'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { App, Button, Empty, Popconfirm, Space, Tabs, Typography } from 'antd'
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { triggerAnalyze, triggerIndex } from '../api/repos'
import { QueryErrorAlert } from '../components/QueryErrorAlert'
import { requireRepoId, useRepoStats } from '../hooks/queries'
import { useRepo } from '../repo/use-repo'
import { useWsEvent } from '../ws/use-ws'
import { DistributionCards } from './dashboard/DistributionCards'
import { IndexProgressPanel, type IndexProgressState } from './dashboard/IndexProgressPanel'
import { RecentRunsTable } from './dashboard/RecentRunsTable'
import { StatCards } from './dashboard/StatCards'

export default function DashboardPage(): ReactNode {
  const { repoId, repos, loading: reposLoading } = useRepo()
  const { message } = App.useApp()
  const queryClient = useQueryClient()
  const [progress, setProgress] = useState<IndexProgressState | null>(null)
  const [logs, setLogs] = useState<string[]>([])
  const statsQuery = useRepoStats(repoId, progress !== null ? 2000 : false)

  useWsEvent((event) => {
    if (event.type === 'log') {
      setLogs((prev) => [...prev.slice(-49), `[${event.level}] ${event.message}`])
      return
    }
    const forCurrent = repoId !== null && String(event.repo_id) === String(repoId)
    if (!forCurrent) return
    if (event.type === 'index_progress') {
      setProgress({ done: event.done, total: event.total, phase: event.phase })
      return
    }
    if (event.type === 'index_done') {
      setProgress(null)
      message.success(
        `索引完成：${event.stats.files_changed} 个文件 / ${event.stats.symbols} 个符号 / ${event.stats.edges} 条边`,
      )
      void queryClient.invalidateQueries({ queryKey: ['repos'] })
      return
    }
    if (event.type === 'task_update') {
      void queryClient.invalidateQueries({ queryKey: ['repos', repoId] })
    }
  })

  const indexMutation = useMutation({
    mutationFn: (full: boolean) => triggerIndex(requireRepoId(repoId), full),
    onSuccess: () => message.success('索引任务已触发'),
  })
  const analyzeMutation = useMutation({
    mutationFn: () => triggerAnalyze(requireRepoId(repoId)),
    onSuccess: (result) => message.success(`已入队 ${result.enqueued} 个 LLM 分析任务`),
  })

  const logRef = useRef<HTMLPreElement | null>(null)
  useEffect(() => {
    const el = logRef.current
    if (el !== null) el.scrollTop = el.scrollHeight
  }, [logs])

  if (!reposLoading && repos.length === 0) {
    return <Empty description="尚未添加仓库：点击顶部「添加」按钮注册一个本地仓库路径" />
  }

  const runs = statsQuery.data?.recent_runs ?? []

  return (
    <div>
      <QueryErrorAlert error={statsQuery.error} context="加载仓库统计" />
      <div className="page-head">
        <Typography.Title level={4} style={{ margin: 0 }}>
          Dashboard
        </Typography.Title>
        <Space wrap>
          <Button
            type="primary"
            icon={<PlayCircleOutlined />}
            disabled={repoId === null}
            loading={indexMutation.isPending}
            onClick={() => indexMutation.mutate(false)}
          >
            增量索引
          </Button>
          <Popconfirm
            title="全量重建"
            description="将重新解析该仓库全部文件，耗时较长，确定继续？"
            okText="重建"
            okButtonProps={{ danger: true }}
            onConfirm={() => indexMutation.mutate(true)}
          >
            <Button
              danger
              icon={<ReloadOutlined />}
              disabled={repoId === null}
              loading={indexMutation.isPending}
            >
              全量重建
            </Button>
          </Popconfirm>
          <Button
            icon={<ThunderboltOutlined />}
            disabled={repoId === null}
            loading={analyzeMutation.isPending}
            onClick={() => analyzeMutation.mutate()}
          >
            LLM 分析
          </Button>
        </Space>
      </div>
      <IndexProgressPanel progress={progress} logs={logs} />
      <Tabs
        defaultActiveKey="overview"
        items={[
          {
            key: 'overview',
            label: '概览',
            children: (
              <Space direction="vertical" size={12} style={{ width: '100%' }}>
                <StatCards stats={statsQuery.data} loading={statsQuery.isLoading} />
                <DistributionCards stats={statsQuery.data} />
              </Space>
            ),
          },
          {
            key: 'runs',
            label: `运行历史（${runs.length}）`,
            children: <RecentRunsTable runs={runs} />,
          },
          {
            key: 'logs',
            label: '事件日志',
            children:
              logs.length === 0 ? (
                <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无事件（等待索引/分析触发）" />
              ) : (
                <pre ref={logRef} className="log-tail is-full">{logs.join('\n')}</pre>
              ),
          },
        ]}
      />
    </div>
  )
}
