import { PlusOutlined } from '@ant-design/icons'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { App, Button, Form, Input, Modal, Select, Space, Typography } from 'antd'
import { useState, type ReactNode } from 'react'
import type { CreateRepoInput, Id, Repo } from '../api/types'
import { createRepo } from '../api/repos'
import { useRepo } from '../repo/use-repo'

function AddRepoModal({ open, onClose }: { open: boolean; onClose: () => void }): ReactNode {
  const { message } = App.useApp()
  const [form] = Form.useForm<CreateRepoInput>()
  const queryClient = useQueryClient()
  const mutation = useMutation({
    mutationFn: (values: CreateRepoInput) => createRepo(values),
    onSuccess: (repo: Repo) => {
      message.success(`已添加仓库：${repo.name}`)
      void queryClient.invalidateQueries({ queryKey: ['repos'] })
      form.resetFields()
      onClose()
    },
  })
  return (
    <Modal
      title="添加仓库"
      open={open}
      onCancel={onClose}
      confirmLoading={mutation.isPending}
      onOk={() => form.submit()}
      destroyOnClose
    >
      <Form form={form} layout="vertical" onFinish={(values) => mutation.mutate(values)}>
        <Form.Item
          name="path"
          label="本地路径"
          rules={[{ required: true, message: '请输入仓库本地路径' }]}
        >
          <Input placeholder="/data2/your-repo" autoFocus />
        </Form.Item>
        <Form.Item name="name" label="名称（可选）">
          <Input placeholder="默认取路径末段" />
        </Form.Item>
        <Form.Item name="git_url" label="Git URL（可选）">
          <Input placeholder="https://github.com/org/repo" />
        </Form.Item>
      </Form>
    </Modal>
  )
}

export function RepoSelector(): ReactNode {
  const { repos, repoId, setRepoId, loading } = useRepo()
  const [open, setOpen] = useState(false)
  return (
    <Space>
      <Typography.Text type="secondary">仓库</Typography.Text>
      <Select<Id>
        style={{ minWidth: 260 }}
        placeholder={loading ? '加载中…' : repos.length === 0 ? '暂无仓库，请添加' : '选择仓库'}
        value={repoId ?? undefined}
        loading={loading}
        showSearch
        optionFilterProp="label"
        onChange={(value) => setRepoId(value)}
        options={repos.map((r) => ({ value: r.id, label: r.name }))}
      />
      <Button icon={<PlusOutlined />} onClick={() => setOpen(true)}>
        添加
      </Button>
      <AddRepoModal open={open} onClose={() => setOpen(false)} />
    </Space>
  )
}
