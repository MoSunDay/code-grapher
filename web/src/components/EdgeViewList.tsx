import { LinkOutlined } from '@ant-design/icons'
import { Empty, List, Space, Tag, Typography } from 'antd'
import type { ReactNode } from 'react'
import type { EdgeSymbolRef, EdgeView, Id } from '../api/types'
import { ConfidenceTag, KindTag, SourceBadge } from './Badges'

interface Props {
  edges: EdgeView[]
  perspective: 'callee' | 'caller'
  onSelect?: (id: Id) => void
}

export function EdgeViewList({ edges, perspective, onSelect }: Props): ReactNode {
  if (edges.length === 0) {
    return (
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description={perspective === 'callee' ? '暂无被调函数' : '暂无调用方'}
      />
    )
  }
  return (
    <List<EdgeView>
      size="small"
      dataSource={edges}
      renderItem={(edge) => {
        const ref: EdgeSymbolRef | null | undefined =
          perspective === 'callee' ? edge.callee : edge.caller
        const resolved = ref !== null && ref !== undefined && ref.id !== undefined
        const name = ref?.name ?? (perspective === 'callee' ? edge.callee_name : '（未知调用方）')
        const location = resolved ? `${ref?.file?.path ?? ''}:${ref?.start_line ?? '?'}` : '未解析'
        return (
          <List.Item key={String(edge.id)} style={{ padding: '6px 0' }}>
            <Space direction="vertical" size={0} style={{ width: '100%' }}>
              <Space wrap size={4}>
                <Typography.Text
                  strong
                  style={resolved ? { color: '#2f54eb', cursor: 'pointer' } : undefined}
                  onClick={() => {
                    if (resolved && onSelect !== undefined) onSelect(ref?.id as Id)
                  }}
                >
                  {resolved && <LinkOutlined style={{ marginRight: 4 }} />}
                  {name}
                </Typography.Text>
                {ref !== null && ref !== undefined && <KindTag kind={ref.kind} />}
                <Tag style={{ marginInlineEnd: 0 }}>边:{edge.kind}</Tag>
                <SourceBadge source={edge.source} />
                <ConfidenceTag confidence={edge.confidence} />
              </Space>
              <Typography.Text type="secondary" style={{ fontSize: 12 }} ellipsis>
                {ref?.qual_path ?? ''} {location}
              </Typography.Text>
            </Space>
          </List.Item>
        )
      }}
    />
  )
}
