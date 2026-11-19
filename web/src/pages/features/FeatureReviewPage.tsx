import { ReloadOutlined } from '@ant-design/icons'
import { Button, Empty, Space, Tabs, Typography } from 'antd'
import { useState, type ReactNode } from 'react'
import { useSelectedRepo } from '../../repo/use-repo'
import { MrTab } from './MrTab'
import { ViewerTab } from './ViewerTab'

/** 仓库理解页：查看（文件树 + 内容）与 MR 记录两个 tab。 */
export default function FeatureReviewPage(): ReactNode {
  const [tab, setTab] = useState('viewer')
  const [mr, setMr] = useState<string | null>(null)
  const repo = useSelectedRepo()
  const featureRepoId = repo?.feature_repo_id ?? null

  if (repo === null) {
    return <Empty description="请先在顶部选择一个仓库" style={{ marginTop: 80 }} />
  }
  if (featureRepoId === null || featureRepoId === '') {
    return <Empty description="该仓库未配置 Git URL，仓库理解不可用。请在上方仓库切换器中补填 Git URL 重新登记，或选择一个通过 URL 注册的仓库" style={{ marginTop: 80 }} />
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      <div className="page-head" style={{ flex: 'none' }}>
        <Typography.Title level={4} style={{ margin: 0 }}>
          仓库理解
        </Typography.Title>
        <Typography.Text type="secondary" copyable style={{ fontSize: 12 }}>
          repo_id {featureRepoId.slice(0, 16)}…
        </Typography.Text>
        <Space>
          <Button
            icon={<ReloadOutlined />}
            onClick={() => window.location.reload()}
            title="刷新页面"
          />
        </Space>
      </div>
      <Tabs
        activeKey={tab}
        onChange={setTab}
        style={{ flex: 1, minHeight: 0 }}
        items={[
          {
            key: 'viewer',
            label: '查看',
            children: <ViewerTab featureRepoId={featureRepoId} mr={mr} onMrChange={setMr} />,
          },
          {
            key: 'mrs',
            label: 'MR 记录',
            children: (
              <MrTab
                featureRepoId={featureRepoId}
                onOpenViewer={(name) => {
                  setMr(name)
                  setTab('viewer')
                }}
              />
            ),
          },
        ]}
      />
    </div>
  )
}
