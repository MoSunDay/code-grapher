import { Alert, Badge, Card, Descriptions, Space, Table, Tag, Typography } from 'antd'
import type { ColumnsType } from 'antd/es/table'
import type { ReactNode } from 'react'
import { QueryErrorAlert } from '../components/QueryErrorAlert'
import { useConfig } from '../hooks/queries'

interface EnvDoc {
  name: string
  desc: string
}

const ENV_DOCS: EnvDoc[] = [
  {
    name: 'TURSO_DATABASE_URL',
    desc: '远端 Turso 数据库地址；未设置时后端使用本地 libsql 文件（db.mode=local）',
  },
  { name: 'TURSO_AUTH_TOKEN', desc: 'Turso 访问令牌，仅存在于后端进程环境变量，API 绝不回显' },
  {
    name: 'OPENCODER_SERVER_URL',
    desc: 'opencode server 地址，用于 LLM 动态调用补齐（hidden_calls 分析）',
  },
  { name: 'OPENCODER_API_KEY', desc: 'opencode server 密钥，仅后端进程可读，API 绝不回显' },
]

const envColumns: ColumnsType<EnvDoc> = [
  {
    title: '环境变量',
    dataIndex: 'name',
    width: 240,
    render: (v: string) => (
      <Typography.Text code copyable>
        {v}
      </Typography.Text>
    ),
  },
  { title: '说明', dataIndex: 'desc' },
]

export default function SettingsPage(): ReactNode {
  const query = useConfig()
  const config = query.data

  return (
    <Space direction="vertical" size={16} style={{ width: '100%' }}>
      <QueryErrorAlert error={query.error} context="加载服务配置" />
      <Card title="服务配置（只读，GET /api/config）" size="small" loading={query.isLoading}>
        {config !== undefined ? (
          <Descriptions
            bordered
            size="small"
            column={{ xs: 1, sm: 2 }}
            items={[
              {
                key: 'mode',
                label: '数据库模式',
                children:
                  config.db.mode === 'remote' ? (
                    <Tag color="geekblue">remote（Turso）</Tag>
                  ) : (
                    <Tag>local（libsql 文件）</Tag>
                  ),
              },
              {
                key: 'url_set',
                label: 'opencode server',
                children: config.opencode.url_set ? (
                  <Badge status="success" text="URL 已配置" />
                ) : (
                  <Badge status="warning" text="URL 未配置" />
                ),
              },
              {
                key: 'concurrency',
                label: 'LLM 并发数',
                children: String(config.opencode.concurrency),
              },
              {
                key: 'version',
                label: '后端版本',
                children: (
                  <Typography.Text code>
                    {config.version === '' ? '-' : config.version}
                  </Typography.Text>
                ),
              },
            ]}
          />
        ) : null}
      </Card>
      <Card title="环境变量说明" size="small">
        <Alert
          type="info"
          showIcon
          style={{ marginBottom: 12 }}
          message="密钥仅存在于后端进程环境变量中，API 不回显任何凭证；前端同样不持有任何密钥。"
        />
        <Table
          size="small"
          pagination={false}
          rowKey="name"
          dataSource={ENV_DOCS}
          columns={envColumns}
        />
        <Typography.Paragraph type="secondary" style={{ marginTop: 12, marginBottom: 0 }}>
          开发模式下，Vite 将 <Typography.Text code>/api</Typography.Text> 与{' '}
          <Typography.Text code>/ws</Typography.Text> 代理到{' '}
          <Typography.Text code>http://127.0.0.1:7800</Typography.Text>（见 vite.config.ts）。
        </Typography.Paragraph>
      </Card>
    </Space>
  )
}
