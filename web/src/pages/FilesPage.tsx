import { Card, Col, Empty, Input, Row } from 'antd'
import { useEffect, useState, type ReactNode } from 'react'
import type { FileMeta } from '../api/types'
import { QueryErrorAlert } from '../components/QueryErrorAlert'
import { useFileContent, useFiles, useSymbols } from '../hooks/queries'
import { useRepo } from '../repo/use-repo'
import { FileDetail } from './files/FileDetail'
import { FileTree } from './files/FileTree'

export default function FilesPage(): ReactNode {
  const { repoId } = useRepo()
  const filesQuery = useFiles(repoId)
  const [filter, setFilter] = useState('')
  const [selected, setSelected] = useState<FileMeta | null>(null)

  useEffect(() => {
    setSelected(null)
    setFilter('')
  }, [repoId])

  const files = filesQuery.data?.files ?? []
  const contentQuery = useFileContent(repoId, selected?.path ?? null)
  const symbolsQuery = useSymbols(
    repoId,
    selected !== null ? { file_id: selected.id, limit: 500 } : null,
  )

  return (
    <Row gutter={[16, 16]}>
      <Col xs={24} md={10} lg={8}>
        <Card size="small" title={`文件（${files.length}）`}>
          <QueryErrorAlert error={filesQuery.error} context="加载文件列表" />
          <Input.Search
            allowClear
            placeholder="按路径过滤，如 crates/cg-core"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            style={{ marginBottom: 8 }}
          />
          <FileTree
            files={files}
            filter={filter}
            selectedId={selected === null ? null : String(selected.id)}
            loading={filesQuery.isLoading}
            onSelect={setSelected}
          />
        </Card>
      </Col>
      <Col xs={24} md={14} lg={16}>
        {selected === null ? (
          <Card size="small">
            <Empty description="在左侧选择一个文件，查看源码与符号" />
          </Card>
        ) : (
          <FileDetail
            file={selected}
            content={contentQuery.data?.content}
            contentLoading={contentQuery.isLoading}
            symbols={symbolsQuery.data?.symbols ?? []}
            symbolsLoading={symbolsQuery.isLoading}
          />
        )}
      </Col>
    </Row>
  )
}
