import { FileOutlined, FolderOutlined } from '@ant-design/icons'
import { Empty, Spin, Tree } from 'antd'
import type { DataNode } from 'antd/es/tree'
import { useMemo, type ReactNode } from 'react'
import type { FileMeta } from '../../api/types'

interface DirBucket {
  dirs: Map<string, DirBucket>
  files: FileMeta[]
}

function emptyBucket(): DirBucket {
  return { dirs: new Map(), files: [] }
}

function buildTreeData(files: FileMeta[]): DataNode[] {
  const root = emptyBucket()
  for (const file of files) {
    const parts = file.path.split('/')
    let current = root
    for (let i = 0; i < parts.length - 1; i += 1) {
      const segment = parts[i]
      if (segment === '') continue
      let next = current.dirs.get(segment)
      if (next === undefined) {
        next = emptyBucket()
        current.dirs.set(segment, next)
      }
      current = next
    }
    current.files.push(file)
  }
  const toNodes = (bucket: DirBucket, prefix: string): DataNode[] => {
    const dirNodes: DataNode[] = [...bucket.dirs.entries()]
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([name, child]) => ({
        key: `dir:${prefix}${name}/`,
        title: name,
        icon: <FolderOutlined />,
        children: toNodes(child, `${prefix}${name}/`),
      }))
    const fileNodes: DataNode[] = [...bucket.files]
      .sort((a, b) => a.path.localeCompare(b.path))
      .map((file) => ({
        key: `file:${String(file.id)}`,
        title: `${file.path.split('/').pop() ?? file.path}（${file.loc} 行）`,
        icon: <FileOutlined />,
        isLeaf: true,
      }))
    return [...dirNodes, ...fileNodes]
  }
  return toNodes(root, '')
}

interface Props {
  files: FileMeta[]
  filter: string
  selectedId: string | null
  loading: boolean
  onSelect: (file: FileMeta) => void
}

export function FileTree({ files, filter, selectedId, loading, onSelect }: Props): ReactNode {
  const normalized = filter.trim().toLowerCase()
  const filtered = useMemo(
    () =>
      normalized === '' ? files : files.filter((f) => f.path.toLowerCase().includes(normalized)),
    [files, normalized],
  )
  const treeData = useMemo(() => buildTreeData(filtered), [filtered])

  if (loading) return <Spin style={{ display: 'block', margin: '24px auto' }} />
  if (files.length === 0)
    return <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无文件（先触发索引）" />
  if (treeData.length === 0)
    return <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="无匹配文件" />

  return (
    <Tree
      key={normalized}
      blockNode
      showIcon
      height={480}
      defaultExpandAll={normalized !== ''}
      treeData={treeData}
      selectedKeys={selectedId === null ? [] : [`file:${selectedId}`]}
      onSelect={(keys) => {
        const key = keys[0]
        if (typeof key !== 'string' || !key.startsWith('file:')) return
        const file = files.find((f) => `file:${String(f.id)}` === key)
        if (file !== undefined) onSelect(file)
      }}
    />
  )
}
