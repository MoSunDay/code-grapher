import { Tag } from 'antd'
import type { ReactNode } from 'react'
import type { EdgeSource } from '../api/types'
import { EDGE_SOURCE_META, KIND_TAG_COLORS } from '../theme'

export function KindTag({ kind }: { kind: string }): ReactNode {
  return <Tag color={KIND_TAG_COLORS[kind]}>{kind}</Tag>
}

export function SourceBadge({ source }: { source: EdgeSource }): ReactNode {
  const meta = EDGE_SOURCE_META[source] ?? { label: source, tagColor: 'default', line: '#8c8c8c' }
  return <Tag color={meta.tagColor}>{meta.label}</Tag>
}

export function ConfidenceTag({
  confidence,
}: {
  confidence: number | null | undefined
}): ReactNode {
  if (typeof confidence !== 'number' || !Number.isFinite(confidence)) return <Tag>-</Tag>
  const pct = Math.round(confidence * 100)
  const color = pct >= 90 ? 'green' : pct >= 60 ? 'gold' : 'red'
  return <Tag color={color}>{pct}%</Tag>
}
