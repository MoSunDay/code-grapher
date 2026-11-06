import { Divider } from 'antd'
import type { ReactNode } from 'react'
import type { EdgeSource, RepoStatsPayload } from '../../api/types'
import { EDGE_SOURCE_META, KIND_NODE_COLORS } from '../../theme'

interface Props {
  stats: RepoStatsPayload | undefined
}

const PALETTE = ['#58a6ff', '#39c5cf', '#ffa657', '#bc8cff', '#7ee787', '#ff7b72']

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
    <div className="dist-row" title={label}>
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

/** 扁平分布面板（替代双卡片）。 */
export function DistributionCards({ stats }: Props): ReactNode {
  const kindEntries = Object.entries(stats?.symbols_by_kind ?? {}).sort((a, b) => b[1] - a[1])
  const kindMax = kindEntries[0]?.[1] ?? 0
  const sourceEntries = Object.entries(stats?.edges_by_source ?? {}) as Array<
    [EdgeSource, number]
  >
  const sourceMax = sourceEntries[0]?.[1] ?? 0
  const edgeKindEntries = Object.entries(stats?.edges_by_kind ?? {}).sort((a, b) => b[1] - a[1])
  const edgeKindMax = edgeKindEntries[0]?.[1] ?? 0
  return (
    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(320px, 1fr))', gap: 12 }}>
      <section className="flat-panel">
        <div className="flat-panel-title">符号分布（symbols_by_kind）</div>
        {kindEntries.length === 0 && <span style={{ color: '#8b949e', fontSize: 12 }}>暂无数据</span>}
        {kindEntries.map(([kind, count], i) => (
          <BarRow
            key={kind}
            label={kind}
            count={count}
            max={kindMax}
            color={KIND_NODE_COLORS[kind] ?? PALETTE[i % PALETTE.length]}
          />
        ))}
      </section>
      <section className="flat-panel">
        <div className="flat-panel-title">调用边分布（edges_by_source / kind）</div>
        {sourceEntries.map(([source, count]) => (
          <BarRow
            key={source}
            label={EDGE_SOURCE_META[source]?.label ?? source}
            count={count}
            max={sourceMax}
            color={EDGE_SOURCE_META[source]?.line ?? '#8b949e'}
          />
        ))}
        {edgeKindEntries.length > 0 && <Divider style={{ margin: '10px 0 8px' }} />}
        {edgeKindEntries.map(([kind, count]) => (
          <BarRow key={kind} label={kind} count={count} max={edgeKindMax} color="#b1bac4" />
        ))}
      </section>
    </div>
  )
}
