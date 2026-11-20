import { PlusOutlined } from '@ant-design/icons'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { App, Button, Collapse, Form, Input, Modal, Select, Space, Typography } from 'antd'
import { useState, type ReactNode } from 'react'
import type { CreateRepoInput, Id, Repo } from '../api/types'
import { createRepo } from '../api/repos'
import { useRepo } from '../repo/use-repo'

interface AddRepoFormValues {
  url?: string
  path?: string
  name?: string
  git_url?: string
}

function AddRepoModal({ open, onClose }: { open: boolean; onClose: () => void }): ReactNode {
  const { message } = App.useApp()
  const [form] = Form.useForm<AddRepoFormValues>()
  const queryClient = useQueryClient()
  const mutation = useMutation({
    mutationFn: (values: CreateRepoInput) => createRepo(values),
    onSuccess: (repo: Repo) => {
      if (repo.status === 'cloning') {
        message.success(`注册成功：${repo.name}，正在克隆并开始解析…`)
      } else {
        message.success(`已添加仓库：${repo.name}，已开始解析`)
      }
      void queryClient.invalidateQueries({ queryKey: ['repos'] })
      form.resetFields()
      onClose()
    },
  })

  const onFinish = (values: AddRepoFormValues): void => {
    const url = values.url?.trim() ?? ''
    const path = values.path?.trim() ?? ''
    const name = values.name?.trim() === '' ? undefined : values.name?.trim()
    if (url !== '') {
      if (path !== '') {
        message.info('同时填写了 URL 与本地路径，按 URL 模式注册（本地路径已忽略）')
      }
      mutation.mutate({ url, name })
    } else {
      mutation.mutate({ path, name, git_url: values.git_url?.trim() || undefined })
    }
  }

  return (
    <Modal
      title="添加仓库"
      open={open}
      onCancel={onClose}
      confirmLoading={mutation.isPending}
      onOk={() => form.submit()}
      destroyOnHidden
      afterClose={() => form.resetFields()}
    >
      <Form form={form} layout="vertical" onFinish={onFinish}>
        <Form.Item
          name="url"
          label="仓库 URL"
          validateFirst
          dependencies={['path']}
          rules={[
            ({ getFieldValue }) => ({
              validator(_, value: string | undefined) {
                if ((value ?? '').trim() !== '' || (getFieldValue('path') ?? '').trim() !== '') {
                  return Promise.resolve()
                }
                return Promise.reject(new Error('请输入仓库 URL，或展开高级选项填写本地路径'))
              },
            }),
          ]}
        >
          <Input placeholder="https://github.com/org/repo" autoFocus />
        </Form.Item>
        <Form.Item name="name" label="名称（可选）">
          <Input placeholder="默认取 URL 末段" />
        </Form.Item>
        <Collapse
          ghost
          items={[
            {
              key: 'advanced',
              label: '高级：本地已有 checkout',
              children: (
                <>
                  <Form.Item
                    name="path"
                    label="本地路径"
                    dependencies={['url']}
                    rules={[
                      ({ getFieldValue }) => ({
                        validator(_, value: string | undefined) {
                          if ((value ?? '').trim() !== '' || (getFieldValue('url') ?? '').trim() !== '') {
                            return Promise.resolve()
                          }
                          return Promise.reject(new Error('本地路径与仓库 URL 至少填一项'))
                        },
                      }),
                    ]}
                  >
                    <Input placeholder="/data2/your-repo（已存在的本地克隆）" />
                  </Form.Item>
                  <Form.Item name="git_url" label="Git URL（可选，本地模式用于仓库理解身份）">
                    <Input placeholder="https://github.com/org/repo" />
                  </Form.Item>
                </>
              ),
            },
          ]}
        />
      </Form>
    </Modal>
  )
}

function repoLabel(name: string, status?: string): string {
  if (status === 'cloning') return `${name}（克隆中…）`
  if (status === 'failed') return `${name}（克隆失败）`
  return name
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
        options={repos.map((r) => ({ value: r.id, label: repoLabel(r.name, r.status) }))}
      />
      <Button icon={<PlusOutlined />} onClick={() => setOpen(true)}>
        添加
      </Button>
      <AddRepoModal open={open} onClose={() => setOpen(false)} />
    </Space>
  )
}
