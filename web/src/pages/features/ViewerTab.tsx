import { BranchesOutlined, CheckCircleOutlined } from '@ant-design/icons'
import { useQuery } from '@tanstack/react-query'
import { Button, Empty, Select, Space, Tag, Typography } from 'antd'
import { useState, type ReactNode } from 'react'
import { getTree, listMrs } from '../../api/features'
import { QueryErrorAlert } from '../../components/QueryErrorAlert'
import { ApproveModal } from './ApproveModal'
import { ContentView } from './ContentView'
import { DocsTree } from './DocsTree'

/**
 * 查看 tab：左侧 MR 分支文件树（draft 橙标），右侧内容渲染；
 * 顶部 MR 上下文与 approve 入口。
 */
export function ViewerTab({
  featureRepoId,
  mr,
  onMrChange,
}: {
  featureRepoId: string
  mr: string | null
  onMrChange: (mr: string | null) => void
}): ReactNode {
  const [selectedFile, setSelectedFile] = useState<string | null>(null)
  const [approveOpen, setApproveOpen] = useState(false)

  const mrs = useQuery({
    queryKey: ['feature-mrs', featureRepoId],
    queryFn: () => listMrs(featureRepoId),
    refetchInterval: 15_000,
  })
  const tree = useQuery({
    queryKey: ['feature-tree', featureRepoId, mr],
    queryFn: () => getTree(featureRepoId, mr ?? undefined),
  })

  const current = mrs.data?.mrs.find((item) => item.name === mr) ?? null
  const file = tree.data?.files.find((f) => f.path === selectedFile) ?? null

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 12, height: '100%' }}>
      <Space wrap style={{ flex: 'none' }}>
        <Select<'' | string>
          style={{ minWidth: 320 }}
          value={mr ?? ''}
          onChange={(value) => {
            setSelectedFile(null)
            onMrChange(value === '' ? null : value)
          }}
          options={[
            { value: '', label: 'main（稳定态）' },
            ...(mrs.data?.mrs ?? []).map((item) => ({
              value: item.name,
              label: `mr/${item.name}（${item.state === 'open' ? '草稿' : '已合并'}）`,
            })),
          ]}
          popupMatchSelectWidth={false}
        />
        {current !== null && (
          <>
            <Tag
              icon={<BranchesOutlined />}
              color={current.state === 'open' ? 'orange' : 'green'}
            >
              {current.state === 'open' ? 'open' : 'merged'} · ahead {current.ahead} / behind{' '}
              {current.behind}
            </Tag>
            {current.state === 'open' && (
              <Button type="primary" danger onClick={() => setApproveOpen(true)}>
                Approve（合入 main）
              </Button>
            )}
            {current.reviewer !== null && (
              <Typography.Text type="secondary">reviewer：{current.reviewer}</Typography.Text>
            )}
          </>
        )}
      </Space>

      <QueryErrorAlert error={tree.error} context="加载文件树" />
      {tree.data !== undefined && tree.data.files.length === 0 && (
        <Empty description="该分支还没有文件（等待 periodic draft 写入）" style={{ marginTop: 60 }} />
      )}
      {tree.data !== undefined && tree.data.files.length > 0 && (
        <div style={{ flex: 1, minHeight: 0, display: 'flex', gap: 12 }}>
          <div
            className="flat-panel"
            style={{ width: 340, flex: 'none', overflow: 'auto', padding: '8px 4px' }}
          >
            <DocsTree
              files={tree.data.files}
              selected={selectedFile}
              onSelect={(path) => setSelectedFile(path)}
            />
            {file?.state === 'draft' && (
              <Typography.Text type="warning" style={{ fontSize: 12, padding: '0 8px' }}>
                橙色文件为该 MR 相对 main 的改动
              </Typography.Text>
            )}
          </div>
          <div className="flat-panel" style={{ flex: 1, minWidth: 0, overflow: 'hidden' }}>
            <ContentView featureRepoId={featureRepoId} mr={mr} path={selectedFile} />
          </div>
        </div>
      )}

      <ApproveModal
        featureRepoId={featureRepoId}
        name={mr}
        expectedTip={current?.tip}
        open={approveOpen}
        onClose={() => setApproveOpen(false)}
      />
      {tree.data?.mr === null && tree.data?.files.length !== 0 && (
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          <CheckCircleOutlined /> 正在浏览 main（已合并的稳定态）
        </Typography.Text>
      )}
    </div>
  )
}
