import { ReloadOutlined } from '@ant-design/icons'
import { useQuery } from '@tanstack/react-query'
import { Button, Empty, Space, Table, Tabs, Tag, Typography } from 'antd'
import { useState, type ReactNode } from 'react'
import { listChangelog, listFeatures } from '../../api/features'
import { QueryErrorAlert } from '../../components/QueryErrorAlert'
import { useSelectedRepo } from '../../repo/use-repo'
import { FeatureDetailDrawer } from './FeatureDetailDrawer'

function FeatureList({ featureRepoId }: { featureRepoId: string }): ReactNode {
  const [selected, setSelected] = useState<string | null>(null)
  const features = useQuery({
    queryKey: ['features', featureRepoId],
    queryFn: () => listFeatures(featureRepoId),
    refetchInterval: 15_000,
  })
  return (
    <Space direction="vertical" size={12} style={{ width: '100%' }}>
      <QueryErrorAlert error={features.error} context="加载功能列表" />
      <Table
        rowKey="name"
        size="small"
        loading={features.isLoading}
        dataSource={features.data?.features ?? []}
        pagination={false}
        onRow={(record) => ({ onClick: () => setSelected(record.name) })}
        columns={[
          {
            title: '功能',
            dataIndex: 'name',
            render: (name: string) => <Typography.Text strong>{name}</Typography.Text>,
          },
          {
            title: '状态',
            dataIndex: 'state',
            width: 110,
            render: (state: string) => (
              <Tag color={state === 'draft' ? 'orange' : 'green'}>{state}</Tag>
            ),
          },
          {
            title: '内容哈希',
            dataIndex: 'hash',
            width: 160,
            render: (hash: string | null) =>
              hash === null ? '-' : <Typography.Text code>{hash.slice(0, 12)}…</Typography.Text>,
          },
        ]}
      />
      <FeatureDetailDrawer
        featureRepoId={featureRepoId}
        name={selected}
        onClose={() => setSelected(null)}
      />
    </Space>
  )
}

function ChangelogPanel({ featureRepoId }: { featureRepoId: string }): ReactNode {
  const changelog = useQuery({
    queryKey: ['feature-changelog', featureRepoId],
    queryFn: () => listChangelog(featureRepoId),
  })
  if (changelog.isLoading) return <Typography.Text type="secondary">加载中…</Typography.Text>
  return (
    <Space direction="vertical" size={12} style={{ width: '100%' }}>
      <QueryErrorAlert error={changelog.error} context="加载 changelog" />
      {(changelog.data?.changelog ?? []).length === 0 && (
        <Empty description="暂无审核记录" />
      )}
      {(changelog.data?.changelog ?? []).map((entry) => (
        <div key={entry.file}>
          <Typography.Title level={5} style={{ marginBottom: 4 }}>
            {entry.file}
          </Typography.Title>
          <pre className="code-block">{entry.markdown}</pre>
        </div>
      ))}
    </Space>
  )
}

export default function FeatureReviewPage(): ReactNode {
  const repo = useSelectedRepo()
  const featureRepoId = repo?.feature_repo_id ?? null

  if (repo === null) {
    return <Empty description="请先在顶部选择一个仓库" />
  }
  if (featureRepoId === null || featureRepoId === '') {
    return <Empty description="该仓库未配置可用 git_url，无法使用功能评审" />
  }

  return (
    <div>
      <div className="page-head">
        <Typography.Title level={4} style={{ margin: 0 }}>
          功能评审 · {repo.name}
        </Typography.Title>
        <Space>
          <Typography.Text type="secondary" copyable style={{ fontSize: 12 }}>
            repo_id {featureRepoId.slice(0, 16)}…
          </Typography.Text>
          <Button
            icon={<ReloadOutlined />}
            onClick={() => window.location.reload()}
            title="刷新页面"
          />
        </Space>
      </div>
      <Tabs
        defaultActiveKey="features"
        items={[
          {
            key: 'features',
            label: '功能',
            children: <FeatureList featureRepoId={featureRepoId} />,
          },
          {
            key: 'changelog',
            label: 'Changelog',
            children: <ChangelogPanel featureRepoId={featureRepoId} />,
          },
        ]}
      />
    </div>
  )
}
