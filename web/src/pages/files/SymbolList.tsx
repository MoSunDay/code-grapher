import { List, Space, Tag, Typography } from 'antd'
import type { ReactNode } from 'react'
import type { Symbol as SymbolEntry } from '../../api/types'
import { KindTag } from '../../components/Badges'

export function SymbolList({ symbols }: { symbols: SymbolEntry[] }): ReactNode {
  return (
    <List
      size="small"
      dataSource={symbols}
      locale={{ emptyText: '该文件暂无符号' }}
      renderItem={(symbol) => (
        <List.Item key={String(symbol.id)} style={{ padding: '4px 0' }}>
          <Space wrap size={6}>
            <KindTag kind={symbol.kind} />
            <Typography.Text strong>{symbol.name}</Typography.Text>
            <Typography.Text type="secondary" code ellipsis style={{ maxWidth: 420 }}>
              {symbol.qual_path}
            </Typography.Text>
            <Typography.Text type="secondary">
              L{symbol.start_line}-{symbol.end_line}
            </Typography.Text>
            {symbol.is_pub && (
              <Tag color="green" style={{ marginInlineEnd: 0 }}>
                pub
              </Tag>
            )}
          </Space>
        </List.Item>
      )}
    />
  )
}
