import { Card, Descriptions, Empty, Skeleton, Space, Tag, Typography } from 'antd'
import { useMemo, type ReactNode } from 'react'
import type { FileMeta, Symbol as SymbolEntry } from '../../api/types'
import { fmtNumber, fmtTime } from '../../lib/format'
import { SourceViewer } from './SourceViewer'
import { SymbolList } from './SymbolList'

interface Props {
  file: FileMeta
  content: string | undefined
  contentLoading: boolean
  symbols: SymbolEntry[]
  symbolsLoading: boolean
}

export function FileDetail({
  file,
  content,
  contentLoading,
  symbols,
  symbolsLoading,
}: Props): ReactNode {
  const symbolLines = useMemo(() => new Set(symbols.map((s) => s.start_line)), [symbols])
  return (
    <Space direction="vertical" size={16} style={{ width: '100%' }}>
      <Card size="small">
        <Descriptions
          size="small"
          column={3}
          items={[
            {
              key: 'path',
              label: '路径',
              span: 2,
              children: (
                <Typography.Text code copyable>
                  {file.path}
                </Typography.Text>
              ),
            },
            {
              key: 'lang',
              label: '语言',
              children: <Tag>{file.lang === '' ? '-' : file.lang}</Tag>,
            },
            { key: 'loc', label: '行数', children: fmtNumber(file.loc) },
            { key: 'count', label: '符号数', children: fmtNumber(file.symbol_count) },
            { key: 'mtime', label: '修改时间', children: fmtTime(file.mtime) },
          ]}
        />
      </Card>
      <Card size="small" title="源码">
        {content === undefined ? (
          contentLoading ? (
            <Skeleton active />
          ) : (
            <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="内容不可用（可能加载失败）" />
          )
        ) : (
          <SourceViewer content={content} symbolLines={symbolLines} />
        )}
      </Card>
      <Card size="small" title={`符号（${symbols.length}）`}>
        {symbolsLoading && symbols.length === 0 ? (
          <Skeleton active />
        ) : (
          <SymbolList symbols={symbols} />
        )}
      </Card>
    </Space>
  )
}
