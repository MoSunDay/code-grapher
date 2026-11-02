import { Card, Col, Divider, Empty, Row } from 'antd'
import type { ReactNode } from 'react'
import type { EdgeSource, RepoStatsPayload } from '../../api/types'
import { EDGE_SOURCE_META, KIND_NODE_COLORS } from '../../theme'

function BarRow({
  label,
  count,
  max,
  color,
}: {
  label: string
  count: number
  max: number
  color: string
}): ReactNode {
  const width = max > 0 ? Math.max(2, Math.round((count / max) * 100)) : 0
  return (
    <div className="dist-row">
      <span className="dist-label" title={label}>
        {label}
      </span>
      <div className="dist-track">
        <div className="dist-fill" style={{ width: `${width}%`, background: color }} />
      </div>
      <span className="dist-count">{count.toLocaleString('zh-CN')}</span>
    </div>
  )
}

function sortedEntries(record: Record<string, number> | undefined): Array<[string, number]> {
  return Object.entries(record ?? {})
    .filter(([, v]) => typeof v === 'number')
    .sort((a, b) => b[1] - a[1])
}

export function DistributionCards({ stats }: { stats: RepoStatsPayload | undefined }): ReactNode {
  const kindEntries = sortedEntries(stats?.symbols_by_kind)
  const sourceEntries: Array<[EdgeSource, number]> = [
    ['ast', stats?.edges_by_source.ast ?? 0],
    ['llm', stats?.edges_by_source.llm ?? 0],
  ]
  const edgeKindEntries = sortedEntries(stats?.edges_by_kind)
  const kindMax = Math.max(1, ...kindEntries.map(([, v]) => v))
  const sourceMax = Math.max(1, ...sourceEntries.map(([, v]) => v))
  const edgeKindMax = Math.max(1, ...edgeKindEntries.map(([, v]) => v))
  const palette = Object.values(KIND_NODE_COLORS)

  return (
    <Row gutter={[16, 16]}>
      <Col xs={24} lg={12}>
        <Card title="符号类型分布（symbols_by_kind）" size="small">
          {kindEntries.length === 0 ? (
            <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无数据" />
          ) : (
            kindEntries.map(([kind, count], i) => (
              <BarRow
                key={kind}
                label={kind}
                count={count}
                max={kindMax}
                color={KIND_NODE_COLORS[kind] ?? palette[i % palette.length]}
              />
            ))
          )}
        </Card>
      </Col>
      <Col xs={24} lg={12}>
        <Card title="调用边分布（edges_by_source / kind）" size="small">
          {sourceEntries.map(([source, count]) => (
            <BarRow
              key={source}
              label={EDGE_SOURCE_META[source]?.label ?? source}
              count={count}
              max={sourceMax}
              color={EDGE_SOURCE_META[source]?.line ?? '#8c8c8c'}
            />
          ))}
          {edgeKindEntries.length > 0 && <Divider style={{ margin: '10px 0 8px' }} />}
          {edgeKindEntries.map(([kind, count]) => (
            <BarRow key={kind} label={kind} count={count} max={edgeKindMax} color="#bfbfbf" />
          ))}
        </Card>
      </Col>
    </Row>
  )
}
