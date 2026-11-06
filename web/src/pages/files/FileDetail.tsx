import { Empty, Skeleton, Tabs, Tag, Typography } from 'antd'
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

/** 单文件视图：扁平元信息头 + Tabs（源码 / 符号），不再卡片套卡片。 */
export function FileDetail({
  file,
  content,
  contentLoading,
  symbols,
  symbolsLoading,
}: Props): ReactNode {
  const symbolLines = useMemo(() => new Set(symbols.map((s) => s.start_line)), [symbols])
  return (
    <div>
      <div className="page-head" style={{ marginBottom: 12 }}>
        <Typography.Text code copyable style={{ fontSize: 13 }}>
          {file.path}
        </Typography.Text>
        {file.lang !== '' && <Tag>{file.lang}</Tag>}
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          {fmtNumber(file.loc)} 行 · {fmtNumber(file.symbol_count)} 符号 · 修改于 {fmtTime(file.mtime)}
        </Typography.Text>
      </div>
      <Tabs
        defaultActiveKey="source"
        items={[
          {
            key: 'source',
            label: '源码',
            children:
              content === undefined ? (
                contentLoading ? (
                  <Skeleton active paragraph={{ rows: 10 }} />
                ) : (
                  <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="内容不可用（可能加载失败）" />
                )
              ) : (
                <SourceViewer content={content} symbolLines={symbolLines} />
              ),
          },
          {
            key: 'symbols',
            label: `符号（${symbols.length}）`,
            children:
              symbolsLoading && symbols.length === 0 ? (
                <Skeleton active paragraph={{ rows: 6 }} />
              ) : symbols.length === 0 ? (
                <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="该文件没有解析出符号" />
              ) : (
                <SymbolList symbols={symbols} />
              ),
          },
        ]}
      />
    </div>
  )
}
