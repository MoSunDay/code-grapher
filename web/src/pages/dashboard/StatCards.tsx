import {
  ClockCircleOutlined,
  ClusterOutlined,
  FileOutlined,
  ShareAltOutlined,
} from '@ant-design/icons'
import { Card, Col, Row, Statistic } from 'antd'
import type { ReactNode } from 'react'
import type { RepoStatsPayload } from '../../api/types'
import { fmtNumber } from '../../lib/format'

interface Props {
  stats: RepoStatsPayload | undefined
  loading: boolean
}

export function StatCards({ stats, loading }: Props): ReactNode {
  const totals = stats?.totals
  const pending = (stats?.task_status.pending ?? 0) + (stats?.task_status.running ?? 0)
  return (
    <Row gutter={[16, 16]}>
      <Col xs={24} sm={12} lg={6}>
        <Card loading={loading}>
          <Statistic title="文件" value={fmtNumber(totals?.files)} prefix={<FileOutlined />} />
        </Card>
      </Col>
      <Col xs={24} sm={12} lg={6}>
        <Card loading={loading}>
          <Statistic
            title="符号"
            value={fmtNumber(totals?.symbols)}
            prefix={<ClusterOutlined />}
            suffix={
              totals !== undefined ? (
                <span style={{ fontSize: 14 }}>（函数 {fmtNumber(totals.functions)}）</span>
              ) : null
            }
          />
        </Card>
      </Col>
      <Col xs={24} sm={12} lg={6}>
        <Card loading={loading}>
          <Statistic
            title="调用边"
            value={fmtNumber(totals?.edges)}
            prefix={<ShareAltOutlined />}
          />
        </Card>
      </Col>
      <Col xs={24} sm={12} lg={6}>
        <Card loading={loading}>
          <Statistic
            title="进行中任务"
            value={fmtNumber(pending)}
            prefix={<ClockCircleOutlined />}
            valueStyle={pending > 0 ? { color: '#fa8c16' } : undefined}
          />
        </Card>
      </Col>
    </Row>
  )
}
