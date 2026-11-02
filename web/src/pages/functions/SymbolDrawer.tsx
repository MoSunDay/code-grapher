import { Alert, Descriptions, Drawer, Space, Spin, Tabs, Typography } from 'antd'
import type { ReactNode } from 'react'
import type { Id } from '../../api/types'
import { extractErrorMessage } from '../../api/client'
import { EdgeViewList } from '../../components/EdgeViewList'
import { KindTag } from '../../components/Badges'
import { useSymbolDetail } from '../../hooks/queries'
import { CallGraphPanel } from './CallGraphPanel'

interface Props {
  symbolId: Id | null
  onClose: () => void
  onSelect: (id: Id) => void
}

export function SymbolDrawer({ symbolId, onClose, onSelect }: Props): ReactNode {
  const query = useSymbolDetail(symbolId)
  const detail = query.data
  const symbol = detail?.symbol

  const title =
    symbol !== undefined ? (
      <Space>
        <Typography.Text strong>{symbol.name}</Typography.Text>
        <KindTag kind={symbol.kind} />
      </Space>
    ) : (
      '符号详情'
    )

  return (
    <Drawer open={symbolId !== null} onClose={onClose} width={720} title={title}>
      {query.isLoading && <Spin style={{ display: 'block', margin: '48px auto' }} />}
      {query.isError && (
        <Alert
          type="error"
          showIcon
          message="加载符号详情失败"
          description={extractErrorMessage(query.error)}
        />
      )}
      {symbol !== undefined && detail !== undefined && (
        <Space direction="vertical" size={16} style={{ width: '100%' }}>
          <Descriptions
            size="small"
            bordered
            column={1}
            items={[
              { key: 'name', label: '名称', children: symbol.name },
              { key: 'kind', label: '类型', children: <KindTag kind={symbol.kind} /> },
              {
                key: 'qual',
                label: '限定路径',
                children: <Typography.Text code>{symbol.qual_path}</Typography.Text>,
              },
              {
                key: 'file',
                label: '位置',
                children: `${symbol.file?.path ?? '-'} · L${symbol.start_line}-L${symbol.end_line}`,
              },
              { key: 'vis', label: '可见性', children: symbol.is_pub ? 'public' : '私有' },
            ]}
          />
          <Tabs
            defaultActiveKey="callees"
            items={[
              {
                key: 'callers',
                label: `调用方（${detail.callers.length}）`,
                children: (
                  <EdgeViewList edges={detail.callers} perspective="caller" onSelect={onSelect} />
                ),
              },
              {
                key: 'callees',
                label: `被调（${detail.callees.length}）`,
                children: (
                  <EdgeViewList edges={detail.callees} perspective="callee" onSelect={onSelect} />
                ),
              },
              {
                key: 'graph',
                label: '调用图',
                children: symbolId !== null ? <CallGraphPanel symbolId={symbolId} /> : null,
              },
            ]}
          />
        </Space>
      )}
    </Drawer>
  )
}
