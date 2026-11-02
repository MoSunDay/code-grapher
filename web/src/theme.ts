import type { ThemeConfig } from 'antd'
import type { EdgeSource, SymbolKind } from './api/types'

export const appTheme: ThemeConfig = {
  token: { colorPrimary: '#2f54eb', borderRadius: 6 },
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

export const KIND_NODE_COLORS: Record<string, string> = {
  function: '#2f54eb',
  method: '#597ef7',
  closure: '#13c2c2',
  struct: '#faad14',
  enum: '#fa8c16',
  trait: '#722ed1',
  type: '#8c8c8c',
  const: '#52c41a',
  static: '#a0d911',
  macro: '#eb2f96',
  impl: '#fa541c',
  mod: '#bfbfbf',
}

export const EDGE_SOURCE_META: Record<
  EdgeSource,
  { label: string; tagColor: string; line: string }
> = {
  ast: { label: 'AST', tagColor: 'geekblue', line: '#8c8c8c' },
  llm: { label: 'LLM', tagColor: 'purple', line: '#722ed1' },
}

export const CENTER_NODE_COLOR = '#2f54eb'
export const UNRESOLVED_NODE_COLOR = '#d9d9d9'
