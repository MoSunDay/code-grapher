import { Tree } from 'antd'
import type { DataNode } from 'antd/es/tree'
import { useMemo, type ReactNode } from 'react'
import type { TreeFile } from '../../api/types'

interface DirNode {
  dirs: Map<string, DirNode>
  files: TreeFile[]
  /** Full path of this directory ('' for the root). */
  path: string
}

const emptyDir = (path: string): DirNode => ({ dirs: new Map(), files: [], path })

function insert(root: DirNode, file: TreeFile): void {
  const parts = file.path.split('/')
  let node = root
  let prefix = ''
  for (const part of parts.slice(0, -1)) {
    prefix = prefix === '' ? part : `${prefix}/${part}`
    let child = node.dirs.get(part)
    if (child === undefined) {
      child = emptyDir(prefix)
      node.dirs.set(part, child)
    }
    node = child
  }
  node.files.push(file)
}

function toDataNodes(node: DirNode): DataNode[] {
  const dirNodes = [...node.dirs.values()]
    .sort((a, b) => a.path.localeCompare(b.path))
    .map<DataNode>((child) => ({
      key: `dir:${child.path}`,
      title: child.path.split('/').pop(),
      selectable: false,
      children: toDataNodes(child),
    }))
  const fileNodes = [...node.files]
    .sort((a, b) => a.path.localeCompare(b.path))
    .map<DataNode>((file) => ({
      key: file.path,
      title: (
        <span style={file.state === 'draft' ? { color: '#f0883e' } : undefined}>
          {file.path.split('/').pop()}
        </span>
      ),
    }))
  return [...dirNodes, ...fileNodes]
}

function collectDirKeys(nodes: DataNode[]): string[] {
  return nodes.flatMap((node) => {
    const children = (node.children ?? []) as DataNode[]
    const nested = collectDirKeys(children)
    return typeof node.key === 'string' && node.key.startsWith('dir:')
      ? [node.key, ...nested]
      : nested
  })
}

/** VSCode-style file tree; draft (branch-changed) files are orange. */
export function DocsTree({
  files,
  selected,
  onSelect,
}: {
  files: TreeFile[]
  selected: string | null
  onSelect: (path: string) => void
}): ReactNode {
  const data = useMemo(() => {
    const root = emptyDir('')
    for (const file of files) insert(root, file)
    return toDataNodes(root)
  }, [files])
  const defaultExpanded = useMemo(() => collectDirKeys(data), [data])
  return (
    <Tree
      blockNode
      treeData={data}
      selectedKeys={selected === null ? [] : [selected]}
      defaultExpandedKeys={defaultExpanded.length > 0 ? defaultExpanded.slice(0, 24) : undefined}
      onSelect={(keys) => {
        const key = keys[0]
        if (typeof key === 'string' && !key.startsWith('dir:')) onSelect(key)
      }}
    />
  )
}
