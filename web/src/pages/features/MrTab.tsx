import { useQuery } from '@tanstack/react-query'
import { Button, Collapse, Space, Table, Tag, Tooltip, Typography } from 'antd'
import { useState, type ReactNode } from 'react'
import { getBlob, getMrCommits, getMrDiff, listMrs } from '../../api/features'
import type { MrSummary } from '../../api/types'
import { QueryErrorAlert } from '../../components/QueryErrorAlert'
import { fmtTime as formatTime } from '../../lib/format'
import { ApproveModal } from './ApproveModal'
import { ChangelogPanel } from './ChangelogPanel'
import { MarkdownView } from './MarkdownView'

/** MR 记录 tab：分支列表 + 状态 + reviewer + 展开看 commits 与 diff。 */
export function MrTab({
  featureRepoId,
  onOpenViewer,
}: {
  featureRepoId: string
  onOpenViewer: (mr: string | null) => void
}): ReactNode {
  const [approveTarget, setApproveTarget] = useState<MrSummary | null>(null)
  const mrs = useQuery({
    queryKey: ['feature-mrs', featureRepoId],
    queryFn: () => listMrs(featureRepoId),
    refetchInterval: 15_000,
  })

  return (
    <Space direction="vertical" size={12} style={{ width: '100%' }}>
      <QueryErrorAlert error={mrs.error} context="加载 MR 列表" />
      <Table<MrSummary>
        rowKey="name"
        size="small"
        loading={mrs.isLoading}
        dataSource={mrs.data?.mrs ?? []}
        pagination={false}
        expandable={{
          expandedRowRender: (record) => <MrExpand featureRepoId={featureRepoId} mr={record} />,
        }}
        columns={[
          {
            title: '功能（MR）',
            dataIndex: 'name',
            render: (name: string, record) => (
              <Typography.Link onClick={() => onOpenViewer(record.state === 'open' ? name : null)}>
                {name}
              </Typography.Link>
            ),
          },
          {
            title: '状态',
            dataIndex: 'state',
            width: 100,
            render: (state: string) => (
              <Tag color={state === 'open' ? 'orange' : 'green'}>{state}</Tag>
            ),
          },
          { title: 'ahead/behind', width: 130, render: (_, r) => `${r.ahead} / ${r.behind}` },
          { title: '最后更新', dataIndex: 'updated_at', width: 170, render: (t: number) => formatTime(t) },
          {
            title: 'Reviewer',
            dataIndex: 'reviewer',
            width: 120,
            render: (v: string | null) => v ?? '-',
          },
          {
            title: '合并时间',
            dataIndex: 'merged_at',
            width: 170,
            render: (t: number | null) => (t === null ? '-' : formatTime(t)),
          },
          {
            title: '操作',
            width: 110,
            render: (_, record) =>
              record.state === 'open' ? (
                <Button size="small" type="primary" danger onClick={() => setApproveTarget(record)}>
                  Approve
                </Button>
              ) : (
                <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                  {record.merge_commit?.slice(0, 8) ?? ''}
                </Typography.Text>
              ),
          },
        ]}
      />
      <ChangelogPanel featureRepoId={featureRepoId} />
      <ApproveModal
        featureRepoId={featureRepoId}
        name={approveTarget?.name ?? null}
        expectedTip={approveTarget?.tip}
        open={approveTarget !== null}
        onClose={() => setApproveTarget(null)}
      />
    </Space>
  )
}

function MrExpand({ featureRepoId, mr }: { featureRepoId: string; mr: MrSummary }): ReactNode {
  const commits = useQuery({
    queryKey: ['feature-mr-commits', featureRepoId, mr.name],
    queryFn: () => getMrCommits(featureRepoId, mr.name),
  })
  const diff = useQuery({
    queryKey: ['feature-mr-diff', featureRepoId, mr.name],
    queryFn: () => getMrDiff(featureRepoId, mr.name),
  })
  return (
    <Space direction="vertical" size={8} style={{ width: '100%' }}>
      {mr.changelog !== null && (
        <Collapse
          size="small"
          items={[
            {
              key: 'changelog',
              label: (
                <Tooltip title={mr.changelog}>
                  <Typography.Text style={{ fontSize: 12 }}>
                    changelog · {baseName(mr.changelog)}
                  </Typography.Text>
                </Tooltip>
              ),
              children: <ChangelogBlob featureRepoId={featureRepoId} path={mr.changelog} />,
            },
          ]}
        />
      )}
      <QueryErrorAlert error={commits.error} context="加载 MR commits" />
      <Typography.Text type="secondary" style={{ fontSize: 12 }}>
        分支 commits（{pendingLabel(commits.data?.commits.length, commits.isPending)}）
      </Typography.Text>
      {commits.isPending && (
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          加载 commits…
        </Typography.Text>
      )}
      {(commits.data?.commits ?? []).map((commit) => (
        <Typography.Text key={commit.hash} style={{ fontSize: 12 }} code>
          {commit.hash.slice(0, 8)} {commit.author} {formatTime(commit.timestamp)} {commit.subject}
        </Typography.Text>
      ))}
      <QueryErrorAlert error={diff.error} context="加载 MR diff" />
      <Typography.Text type="secondary" style={{ fontSize: 12 }}>
        diff（main...mr/{mr.name}）
      </Typography.Text>
      {diff.isPending ? (
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          加载 diff…
        </Typography.Text>
      ) : (
        <pre
          style={{
            margin: 0,
            maxHeight: 260,
            overflow: 'auto',
            fontSize: 12,
            whiteSpace: 'pre-wrap',
          }}
        >
          {diff.data?.diff ?? '（无 diff）'}
        </pre>
      )}
    </Space>
  )
}

function ChangelogBlob({
  featureRepoId,
  path,
}: {
  featureRepoId: string
  path: string | null
}): ReactNode {
  const blob = useQuery({
    queryKey: ['feature-blob', featureRepoId, null, path],
    queryFn: () => getBlob(featureRepoId, path ?? ''),
    enabled: path !== null,
  })
  if (blob.isPending) {
    return (
      <Typography.Text type="secondary" style={{ fontSize: 12 }}>
        加载 changelog…
      </Typography.Text>
    )
  }
  return (
    <Space direction="vertical" size={8} style={{ width: '100%' }}>
      <QueryErrorAlert error={blob.error} context="读取 changelog" />
      {blob.data === undefined ? (
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          暂无 changelog 内容
        </Typography.Text>
      ) : (
        <MarkdownView markdown={blob.data.content} />
      )}
    </Space>
  )
}

function baseName(path: string): string {
  const slash = path.lastIndexOf('/')
  return slash === -1 ? path : path.slice(slash + 1)
}

function pendingLabel(count: number | undefined, pending: boolean): number | string {
  if (count !== undefined) return count
  return pending ? '…' : '-'
}
