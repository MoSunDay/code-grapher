import {
  ClockCircleOutlined,
  ClusterOutlined,
  FileOutlined,
  ShareAltOutlined,
} from '@ant-design/icons'
import { Col, Row, Skeleton } from 'antd'
import type { ReactNode } from 'react'
import type { RepoStatsPayload } from '../../api/types'
import { fmtNumber } from '../../lib/format'

interface Props {
  stats: RepoStatsPayload | undefined
  loading: boolean
}

/** 扁平指标块（无卡片边框堆叠，opencode 风格）。 */
export function StatCards({ stats, loading }: Props): ReactNode {
  const totals = stats?.totals
  const pending = (stats?.task_status.pending ?? 0) + (stats?.task_status.running ?? 0)
  const tiles: Array<{ label: string; value: string; icon: ReactNode; warn?: boolean }> = [
    { label: '文件', value: fmtNumber(totals?.files), icon: <FileOutlined /> },
    {
      label: '符号',
      value:
        totals === undefined
          ? '-'
          : `${fmtNumber(totals.symbols)}（函数 ${fmtNumber(totals.functions)}）`,
      icon: <ClusterOutlined />,
    },
    { label: '调用边', value: fmtNumber(totals?.edges), icon: <ShareAltOutlined /> },
    { label: '进行中任务', value: fmtNumber(pending), icon: <ClockCircleOutlined />, warn: pending > 0 },
  ]
  return (
    <Row gutter={[12, 12]}>
      {tiles.map((tile) => (
        <Col xs={12} lg={6} key={tile.label}>
          <div className="metric-tile">
            <div className="metric-label">
              {tile.icon} {tile.label}
            </div>
            {loading ? (
              <Skeleton active paragraph={false} title={{ width: 96 }} style={{ marginTop: 4 }} />
            ) : (
              <div
                className="metric-value"
                style={tile.warn === true ? { color: '#d29922' } : undefined}
              >
                {tile.value}
              </div>
            )}
          </div>
        </Col>
      ))}
    </Row>
  )
}
