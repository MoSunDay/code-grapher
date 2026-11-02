import { CheckOutlined, StopOutlined } from '@ant-design/icons'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { App, Button, Card, Empty, List, Space, Spin, Tag, Typography } from 'antd'
import type { ReactNode } from 'react'
import type { EdgeView, Id } from '../../api/types'
import { confirmEdge, dismissEdge } from '../../api/tasks'
import { ConfidenceTag, KindTag, SourceBadge } from '../../components/Badges'
import { useUnresolvedEdges } from '../../hooks/queries'

interface Props {
  repoId: Id
}

export function UnresolvedEdges({ repoId }: Props): ReactNode {
  const query = useUnresolvedEdges(repoId)
  const { message } = App.useApp()
  const queryClient = useQueryClient()
  const edges = query.data?.edges ?? []

  const act = useMutation({
    mutationFn: ({ id, action }: { id: Id; action: 'confirm' | 'dismiss' }) =>
      action === 'confirm' ? confirmEdge(id) : dismissEdge(id),
    onSuccess: (_result, variables) => {
      message.success(
        variables.action === 'confirm' ? '已确认该调用边（confidence=1.0）' : '已丢弃该调用边',
      )
      void queryClient.invalidateQueries({ queryKey: ['repos'] })
    },
  })

  return (
    <Card
      size="small"
      title="人审：未通过校验的 LLM 调用边（edges/unresolved）"
      extra={query.isFetching ? <Spin size="small" /> : <Tag>{edges.length} 条待审</Tag>}
    >
      {edges.length === 0 ? (
        <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="没有待人审的调用边" />
      ) : (
        <List<EdgeView>
          size="small"
          dataSource={edges}
          renderItem={(edge) => (
            <List.Item
              key={String(edge.id)}
              style={{ padding: '6px 0' }}
              actions={[
                <Button
                  key="confirm"
                  size="small"
                  type="primary"
                  icon={<CheckOutlined />}
                  loading={act.isPending}
                  onClick={() => act.mutate({ id: edge.id, action: 'confirm' })}
                >
                  确认
                </Button>,
                <Button
                  key="dismiss"
                  size="small"
                  danger
                  icon={<StopOutlined />}
                  loading={act.isPending}
                  onClick={() => act.mutate({ id: edge.id, action: 'dismiss' })}
                >
                  丢弃
                </Button>,
              ]}
            >
              <Space direction="vertical" size={0} style={{ width: '100%' }}>
                <Space wrap size={4}>
                  <Typography.Text strong>{edge.caller?.name ?? '（未知调用方）'}</Typography.Text>
                  <Typography.Text type="secondary">→</Typography.Text>
                  <Typography.Text strong>{edge.callee?.name ?? edge.callee_name}</Typography.Text>
                  {edge.caller !== null && edge.caller !== undefined && (
                    <KindTag kind={edge.caller.kind} />
                  )}
                  <Tag style={{ marginInlineEnd: 0 }}>边:{edge.kind}</Tag>
                  <SourceBadge source={edge.source} />
                  <ConfidenceTag confidence={edge.confidence} />
                </Space>
                <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                  {edge.caller?.qual_path ?? ''}
                </Typography.Text>
              </Space>
            </List.Item>
          )}
        />
      )}
    </Card>
  )
}
