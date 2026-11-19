import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { App, Button, Descriptions, Drawer, Form, Input, Space, Tag, Typography } from 'antd'
import { useState, type ReactNode } from 'react'
import { approveFeature, getFeature, getFeatureDiff } from '../../api/features'
import { QueryErrorAlert } from '../../components/QueryErrorAlert'
import type { FeatureState } from '../../api/types'

function StateTag({ state }: { state: FeatureState }): ReactNode {
  return <Tag color={state === 'draft' ? 'orange' : 'green'}>{state}</Tag>
}

export function FeatureDetailDrawer({
  featureRepoId,
  name,
  onClose,
}: {
  featureRepoId: string
  name: string | null
  onClose: () => void
}): ReactNode {
  const { message } = App.useApp()
  const queryClient = useQueryClient()
  const [reviewer, setReviewer] = useState('')
  const [note, setNote] = useState('')

  const detail = useQuery({
    queryKey: ['feature-detail', featureRepoId, name],
    queryFn: () => getFeature(featureRepoId, name ?? ''),
    enabled: name !== null,
    refetchInterval: 15_000,
  })
  const diff = useQuery({
    queryKey: ['feature-diff', featureRepoId, name],
    queryFn: () => getFeatureDiff(featureRepoId, name ?? ''),
    enabled: name !== null,
  })

  const approve = useMutation({
    mutationFn: () =>
      approveFeature(featureRepoId, name ?? '', {
        reviewer: reviewer.trim(),
        note: note.trim() === '' ? undefined : note.trim(),
        expected_hash: detail.data?.hash,
      }),
    onSuccess: (result) => {
      message.success(`已合入稳定态：${result.commit.slice(0, 10)}`)
      queryClient.invalidateQueries({ queryKey: ['features', featureRepoId] })
      queryClient.invalidateQueries({ queryKey: ['feature-detail', featureRepoId] })
      queryClient.invalidateQueries({ queryKey: ['feature-diff', featureRepoId] })
      queryClient.invalidateQueries({ queryKey: ['feature-changelog', featureRepoId] })
      onClose()
    },
  })

  const data = detail.data
  return (
    <Drawer title={name} width={720} open={name !== null} onClose={onClose} destroyOnHidden>
      <QueryErrorAlert error={detail.error} context="加载功能详情" />
      {data !== undefined && (
        <Space direction="vertical" size={16} style={{ width: '100%' }}>
          <Descriptions size="small" column={2} bordered>
            <Descriptions.Item label="状态">
              <StateTag state={data.state} />
            </Descriptions.Item>
            <Descriptions.Item label="内容哈希">
              <Typography.Text code copyable style={{ fontSize: 12 }}>
                {data.hash}
              </Typography.Text>
            </Descriptions.Item>
          </Descriptions>
          <div>
            <Typography.Title level={5}>feature.json</Typography.Title>
            <pre className="code-block">{JSON.stringify(data.feature, null, 2)}</pre>
          </div>
          <div>
            <Typography.Title level={5}>未提交变更（git diff HEAD）</Typography.Title>
            <pre className="code-block">
              {diff.data !== undefined && diff.data.diff.trim() !== ''
                ? diff.data.diff
                : '（无未提交变更，处于稳定态）'}
            </pre>
          </div>
          {data.state === 'draft' && (
            <Form layout="vertical" onFinish={() => approve.mutate()}>
              <Form.Item label="审核人" required>
                <Input
                  value={reviewer}
                  onChange={(e) => setReviewer(e.target.value)}
                  placeholder="你的名字，将写入 commit 与 changelog"
                />
              </Form.Item>
              <Form.Item label="审核备注">
                <Input.TextArea
                  value={note}
                  onChange={(e) => setNote(e.target.value)}
                  rows={2}
                  placeholder="optional"
                />
              </Form.Item>
              <Space>
                <Button
                  type="primary"
                  htmlType="submit"
                  loading={approve.isPending}
                  disabled={reviewer.trim() === ''}
                >
                  通过并提交（commit → stable）
                </Button>
                <Typography.Text type="secondary">
                  仅提交 feature.json 与 changelog，不携带其他脏文件
                </Typography.Text>
              </Space>
            </Form>
          )}
        </Space>
      )}
    </Drawer>
  )
}
