import { Alert, Empty, Spin, Typography } from 'antd'
import type { Graph } from '@antv/g6'
import { useEffect, useRef, useState, type ReactNode } from 'react'
import type { Id } from '../../api/types'
import { useCallGraph } from '../../hooks/queries'
import { mountCallGraph } from './g6-graph'

export function CallGraphPanel({ symbolId }: { symbolId: Id }): ReactNode {
  const query = useCallGraph(symbolId, 2)
  const containerRef = useRef<HTMLDivElement | null>(null)
  const [renderError, setRenderError] = useState(false)

  useEffect(() => {
    const el = containerRef.current
    const data = query.data
    if (el === null || data === undefined || data.nodes.length === 0) return
    let graph: Graph | null = null
    let disposed = false
    setRenderError(false)
    mountCallGraph(el, data, symbolId)
      .then((g) => {
        if (disposed) {
          void g.destroy()
          return
        }
        graph = g
      })
      .catch(() => {
        setRenderError(true)
      })
    return () => {
      disposed = true
      if (graph !== null) void graph.destroy()
    }
  }, [query.data, symbolId])

  if (query.isLoading) return <Spin style={{ display: 'block', margin: '48px auto' }} />
  if (query.isError) return <Alert type="error" showIcon message="调用图加载失败" />
  if (renderError) return <Alert type="warning" showIcon message="调用图渲染失败，请重试" />
  if ((query.data?.nodes.length ?? 0) === 0) {
    return <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无调用关系（depth=2）" />
  }

  return (
    <div>
      <div ref={containerRef} className="graph-container" />
      <div className="graph-legend">
        <span className="legend-item">
          <span className="legend-dot" style={{ background: '#2f54eb' }} />
          中心符号
        </span>
        <span className="legend-item">
          <span className="legend-line" style={{ borderColor: '#8c8c8c' }} />
          AST 边
        </span>
        <span className="legend-item">
          <span className="legend-line" style={{ borderColor: '#722ed1' }} />
          LLM 边
        </span>
        <span className="legend-item">
          <span
            className="legend-line"
            style={{ borderTopStyle: 'dashed', borderColor: '#bfbfbf' }}
          />
          未解析（灰色虚线）
        </span>
        <Typography.Text type="secondary" style={{ marginLeft: 'auto' }}>
          节点 {query.data?.nodes.length ?? 0} · 边 {query.data?.edges.length ?? 0}
        </Typography.Text>
      </div>
    </div>
  )
}
