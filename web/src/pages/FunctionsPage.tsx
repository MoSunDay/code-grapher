import { Card, Empty, Input, Select, Space, Table, Tag, Typography } from 'antd'
import type { ColumnsType } from 'antd/es/table'
import { useMemo, useState, type ReactNode } from 'react'
import { useSearchParams } from 'react-router-dom'
import type { Id, Symbol as SymbolEntry, SymbolKind } from '../api/types'
import { toId } from '../api/types'
import { KindTag } from '../components/Badges'
import { QueryErrorAlert } from '../components/QueryErrorAlert'
import { useRepo } from '../repo/use-repo'
import { useSymbols } from '../hooks/queries'
import { SYMBOL_KINDS } from '../theme'
import { SymbolDrawer } from './functions/SymbolDrawer'

const PAGE_SIZE = 20

export default function FunctionsPage(): ReactNode {
  const { repoId } = useRepo()
  const [searchParams, setSearchParams] = useSearchParams()
  const [kind, setKind] = useState<SymbolKind | undefined>(undefined)
  const [q, setQ] = useState('')
  const [page, setPage] = useState(1)

  const urlSymbol = searchParams.get('symbol')
  const openId = urlSymbol !== null ? toId(urlSymbol) : null
  const offset = (page - 1) * PAGE_SIZE
  const query = useSymbols(repoId, {
    kind,
    q: q === '' ? undefined : q,
    limit: PAGE_SIZE,
    offset,
  })

  const openSymbol = (id: Id): void => {
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev)
        next.set('symbol', String(id))
        return next
      },
      { replace: true },
    )
  }
  const closeSymbol = (): void => {
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev)
        next.delete('symbol')
        return next
      },
      { replace: true },
    )
  }

  const columns = useMemo<ColumnsType<SymbolEntry>>(
    () => [
      {
        title: '名称',
        dataIndex: 'name',
        width: 240,
        render: (name: string, record) => (
          <Space size={4}>
            <Typography.Text strong>{name}</Typography.Text>
            {record.is_pub && (
              <Tag color="green" style={{ marginInlineEnd: 0 }}>
                pub
              </Tag>
            )}
          </Space>
        ),
      },
      { title: '类型', dataIndex: 'kind', width: 110, render: (k: string) => <KindTag kind={k} /> },
      {
        title: '限定路径',
        dataIndex: 'qual_path',
        ellipsis: true,
        render: (v: string) => (
          <Typography.Text code ellipsis style={{ maxWidth: 360 }}>
            {v}
          </Typography.Text>
        ),
      },
      {
        title: '文件',
        key: 'file',
        ellipsis: true,
        width: 240,
        render: (_, record) => record.file?.path ?? '-',
      },
      {
        title: '行号',
        key: 'lines',
        width: 110,
        render: (_, record) => `L${record.start_line} - L${record.end_line}`,
      },
    ],
    [],
  )

  return (
    <>
      <QueryErrorAlert error={query.error} context="加载符号列表" />
      <Card size="small">
        <Space wrap style={{ marginBottom: 16 }}>
          <Select<SymbolKind>
            allowClear
            placeholder="类型（全部）"
            style={{ minWidth: 150 }}
            value={kind}
            options={SYMBOL_KINDS.map((k) => ({ value: k, label: k }))}
            onChange={(value) => {
              setKind(value)
              setPage(1)
            }}
          />
          <Input.Search
            allowClear
            placeholder="搜索符号名 / 限定路径"
            style={{ width: 280 }}
            onSearch={(value) => {
              setQ(value)
              setPage(1)
            }}
          />
          <Typography.Text type="secondary">点击行查看 callers / callees / 调用图</Typography.Text>
        </Space>
        <Table
          rowKey="id"
          size="small"
          columns={columns}
          dataSource={query.data?.symbols ?? []}
          loading={query.isLoading}
          onRow={(record) => ({
            onClick: () => openSymbol(record.id),
            style: { cursor: 'pointer' },
          })}
          pagination={{
            current: page,
            pageSize: PAGE_SIZE,
            total: query.data?.total ?? 0,
            showSizeChanger: false,
            showTotal: (total) => `共 ${total} 项`,
            onChange: (next) => setPage(next),
          }}
          locale={{
            emptyText: <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无符号" />,
          }}
        />
      </Card>
      <SymbolDrawer symbolId={openId} onClose={closeSymbol} onSelect={openSymbol} />
    </>
  )
}
