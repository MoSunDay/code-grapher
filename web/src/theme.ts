import { theme } from 'antd'
import type { ThemeConfig } from 'antd'
import type { EdgeSource, SymbolKind } from './api/types'

/** 设计语言对齐 opencode server web：GitHub dark 面板 + 等宽字体 + 扁平分隔线。 */
export const appTheme: ThemeConfig = {
  algorithm: theme.darkAlgorithm,
  token: {
    colorPrimary: '#2f81f7',
    colorInfo: '#2f81f7',
    colorSuccess: '#3fb950',
    colorError: '#f85149',
    colorWarning: '#d29922',
    colorBgBase: '#0d1117',
    colorTextBase: '#e6edf3',
    colorBorder: '#30363d',
    colorBorderSecondary: '#21262d',
    borderRadius: 6,
    fontSize: 13,
    fontFamily:
      "ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, 'Liberation Mono', 'PingFang SC', 'Microsoft YaHei', monospace",
  },
  components: {
    Layout: { siderBg: '#161b22', headerBg: '#161b22', bodyBg: '#0d1117', headerHeight: 48 },
    Menu: {
      darkItemBg: '#161b22',
      darkItemColor: '#8b949e',
      darkItemHoverColor: '#e6edf3',
      darkItemSelectedBg: '#1c2230',
      darkItemSelectedColor: '#e6edf3',
    },
    Card: { colorBgContainer: '#161b22' },
    Table: { headerBg: '#161b22' },
    Drawer: { colorBgElevated: '#161b22' },
  },
}

export const SYMBOL_KINDS: SymbolKind[] = [
  'function',
  'method',
  'closure',
  'struct',
  'enum',
  'trait',
  'type',
  'const',
  'static',
  'macro',
  'impl',
  'mod',
]

export const KIND_TAG_COLORS: Record<string, string> = {
  function: 'geekblue',
  method: 'blue',
  closure: 'cyan',
  struct: 'gold',
  enum: 'orange',
  trait: 'purple',
  type: 'default',
  const: 'green',
  static: 'lime',
  macro: 'magenta',
  impl: 'volcano',
  mod: 'default',
}

/** 暗色语法高亮系（GitHub dark），用于 G6 图节点。 */
export const KIND_NODE_COLORS: Record<string, string> = {
  function: '#2f81f7',
  method: '#58a6ff',
  closure: '#39c5cf',
  struct: '#ffa657',
  enum: '#f0883e',
  trait: '#bc8cff',
  type: '#8b949e',
  const: '#3fb950',
  static: '#7ee787',
  macro: '#ff7bfa',
  impl: '#ff7b72',
  mod: '#b1bac4',
}

export const EDGE_SOURCE_META: Record<
  EdgeSource,
  { label: string; tagColor: string; line: string }
> = {
  ast: { label: 'AST', tagColor: 'geekblue', line: '#8b949e' },
  llm: { label: 'LLM', tagColor: 'purple', line: '#bc8cff' },
}

export const CENTER_NODE_COLOR = '#2f81f7'
export const UNRESOLVED_NODE_COLOR = '#6e7681'
