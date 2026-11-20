import { useQuery } from '@tanstack/react-query'
import { Collapse, Empty, Space, Typography } from 'antd'
import type { ReactNode } from 'react'
import { listChangelog } from '../../api/features'
import { QueryErrorAlert } from '../../components/QueryErrorAlert'
import { MarkdownView } from './MarkdownView'

/** MR tab 底部的 changelog 汇总：每个文件的评审记录折叠展示。 */
export function ChangelogPanel({ featureRepoId }: { featureRepoId: string }): ReactNode {
  const changelog = useQuery({
    queryKey: ['feature-changelog', featureRepoId],
    queryFn: () => listChangelog(featureRepoId),
  })
  const entries = changelog.data?.changelog ?? []

  return (
    <Space direction="vertical" size={8} style={{ width: '100%' }}>
      <Typography.Title level={5} style={{ margin: 0 }}>
        Changelog
      </Typography.Title>
      <QueryErrorAlert error={changelog.error} context="加载 changelog" />
      {entries.length === 0 ? (
        <Empty description="暂无 changelog" image={Empty.PRESENTED_IMAGE_SIMPLE} />
      ) : (
        <Collapse
          size="small"
          items={entries.map((entry) => ({
            key: entry.file,
            label: entry.file,
            children: <MarkdownView markdown={entry.markdown} />,
          }))}
        />
      )}
    </Space>
  )
}
