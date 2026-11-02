import { Graph } from '@antv/g6'
import type { CallGraph, Id } from '../../api/types'
import {
  CENTER_NODE_COLOR,
  EDGE_SOURCE_META,
  KIND_NODE_COLORS,
  UNRESOLVED_NODE_COLOR,
} from '../../theme'

interface GraphNodeDatum {
  id: string
  data: { name: string; kind: string; center: boolean }
  [key: string]: unknown
}
interface GraphEdgeDatum {
  id: string
  source: string
  target: string
  data: { edgeSource: string; unresolved: boolean; label: string }
  [key: string]: unknown
}

function field(datum: object, key: string): unknown {
  const holder = datum as Record<string, unknown>
  if (key in holder) return holder[key]
  const inner = holder.data
  if (typeof inner === 'object' && inner !== null) return (inner as Record<string, unknown>)[key]
  return undefined
}

const text = (v: unknown): string =>
  typeof v === 'string' ? v : typeof v === 'number' ? String(v) : ''

const flag = (v: unknown): boolean => v === true

function nodeFill(datum: object): string {
  if (flag(field(datum, 'center'))) return CENTER_NODE_COLOR
  const kind = text(field(datum, 'kind'))
  if (kind === 'unresolved') return UNRESOLVED_NODE_COLOR
  return KIND_NODE_COLORS[kind] ?? '#8c8c8c'
}

export function buildGraphData(
  graph: CallGraph,
  centerId: Id,
): {
  nodes: GraphNodeDatum[]
  edges: GraphEdgeDatum[]
} {
  const nodes: GraphNodeDatum[] = graph.nodes.map((n) => ({
    id: String(n.id),
    data: { name: n.name, kind: String(n.kind), center: n.id === centerId },
  }))
  const edges: GraphEdgeDatum[] = []
  for (const e of graph.edges) {
    const unresolved = e.target_id === null || e.target_id === undefined
    const target = unresolved ? `unresolved:${String(e.id)}` : String(e.target_id)
    if (unresolved) {
      nodes.push({
        id: target,
        data: {
          name: e.callee_name === '' ? '未解析' : e.callee_name,
          kind: 'unresolved',
          center: false,
        },
      })
    }
    edges.push({
      id: String(e.id),
      source: String(e.source_id),
      target,
      data: { edgeSource: e.source, unresolved, label: unresolved ? e.callee_name : '' },
    })
  }
  return { nodes, edges }
}

export async function mountCallGraph(
  container: HTMLElement,
  graph: CallGraph,
  centerId: Id,
): Promise<Graph> {
  const { nodes, edges } = buildGraphData(graph, centerId)
  const instance = new Graph({
    container,
    autoFit: 'view',
    padding: 28,
    behaviors: ['drag-canvas', 'zoom-canvas', 'drag-element'],
    layout: { type: 'radial', unitRadius: 120, preventOverlap: true, nodeSize: 46 },
    data: { nodes, edges },
    node: {
      style: {
        size: (d) => (flag(field(d, 'center')) ? 46 : 32),
        fill: (d) => nodeFill(d),
        stroke: '#ffffff',
        lineWidth: 2,
        labelText: (d) => text(field(d, 'name')),
        labelPlacement: 'bottom',
        labelFontSize: 10,
        labelFill: '#595959',
      },
    },
    edge: {
      style: {
        stroke: (d) =>
          text(field(d, 'edgeSource')) === 'llm'
            ? EDGE_SOURCE_META.llm.line
            : EDGE_SOURCE_META.ast.line,
        lineWidth: 1.2,
        endArrow: true,
        lineDash: (d) => (flag(field(d, 'unresolved')) ? [4, 4] : []),
        labelText: (d) => text(field(d, 'label')),
        labelFontSize: 10,
        labelFill: '#8c8c8c',
      },
    },
  })
  await instance.render()
  return instance
}
