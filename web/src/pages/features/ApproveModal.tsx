import { App, Form, Input, Modal } from 'antd'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { approveMr } from '../../api/features'
import { QueryErrorAlert } from '../../components/QueryErrorAlert'

/** Human approve = merge mr/<name> into main; reviewer is the merge author. */
export function ApproveModal({
  featureRepoId,
  name,
  expectedTip,
  open,
  onClose,
}: {
  featureRepoId: string
  name: string | null
  expectedTip?: string
  open: boolean
  onClose: () => void
}): ReactNode {
  const { message } = App.useApp()
  const queryClient = useQueryClient()
  const [form] = Form.useForm<{ reviewer: string; note?: string }>()

  const approve = useMutation({
    mutationFn: (values: { reviewer: string; note?: string }) =>
      approveMr(featureRepoId, name ?? '', {
        reviewer: values.reviewer.trim(),
        note: values.note?.trim() === '' ? undefined : values.note?.trim(),
        expected_tip: expectedTip,
      }),
    onSuccess: (result) => {
      message.success(`已合入 main：${result.merge_commit.slice(0, 10)}`)
      form.resetFields()
      for (const key of [
        'feature-mrs',
        'feature-tree',
        'feature-blob',
        'feature-changelog',
        'feature-mr-commits',
        'feature-mr-diff',
      ]) {
        queryClient.invalidateQueries({ queryKey: [key, featureRepoId] })
      }
      onClose()
    },
  })

  return (
    <Modal
      title={`Approve MR「${name ?? ''}」`}
      open={open && name !== null}
      confirmLoading={approve.isPending}
      onOk={() => form.submit()}
      onCancel={() => {
        form.resetFields()
        onClose()
      }}
      okText="合入 main"
      okButtonProps={{ danger: true }}
    >
      <QueryErrorAlert error={approve.error} context="合入 main" />
      <Form form={form} layout="vertical" requiredMark onFinish={(values) => approve.mutate(values)}>
        <Form.Item
          name="reviewer"
          label="Reviewer（将作为 merge commit 作者）"
          rules={[{ required: true, message: '填写 reviewer 名字' }]}
        >
          <Input placeholder="你的名字" autoFocus />
        </Form.Item>
        <Form.Item name="note" label="评审备注（可选，写入 changelog）">
          <Input.TextArea rows={2} placeholder="评审结论 / 备注" />
        </Form.Item>
      </Form>
    </Modal>
  )
}
