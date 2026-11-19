import { useMutation, useQueryClient } from '@tanstack/react-query'
import {
  Alert,
  App,
  Badge,
  Descriptions,
  Select,
  Skeleton,
  Space,
  Switch,
  Table,
  Tabs,
  Tag,
  Typography,
} from 'antd'
import type { ColumnsType } from 'antd/es/table'
import type { ReactNode } from 'react'
import { extractErrorMessage } from '../api/client'
import { updateConfig } from '../api/config'
import { QueryErrorAlert } from '../components/QueryErrorAlert'
import { useConfig } from '../hooks/queries'

interface EnvDoc {
  name: string
  desc: string
}

const CONCURRENCY_STORAGE_KEY = 'cg-analyze-concurrency'
const DEFAULT_CONCURRENCY = 2
const CONCURRENCY_OPTIONS = [1, 2, 4, 8, 16]

/** 读取上次启用的并发值（localStorage，缺省 2；非法值回退缺省）。 */
function readStoredConcurrency(): number {
  const raw = window.localStorage.getItem(CONCURRENCY_STORAGE_KEY)
  const value = raw === null ? Number.NaN : Number(raw)
  return CONCURRENCY_OPTIONS.includes(value) ? value : DEFAULT_CONCURRENCY
}

function writeStoredConcurrency(value: number): void {
  window.localStorage.setItem(CONCURRENCY_STORAGE_KEY, String(value))
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
  const { message } = App.useApp()
  const queryClient = useQueryClient()

  const concurrencyMutation = useMutation({
    mutationFn: (analyzeConcurrency: number) => updateConfig(analyzeConcurrency),
    onSuccess: (result) => {
      void queryClient.invalidateQueries({ queryKey: ['config'] })
      if (result.paused) message.success('已暂停 LLM 任务消费（running 任务自然跑完）')
      else message.success(`LLM 并发已调整为 ${result.analyze_concurrency}`)
    },
    onError: (error) => message.error(`更新并发配置失败：${extractErrorMessage(error)}`),
  })

  /** Switch 开 = 恢复上次启用值（localStorage 缺省 2）；关 = 0（暂停消费）。 */
  const toggleConsumption = (enabled: boolean): void => {
    concurrencyMutation.mutate(enabled ? readStoredConcurrency() : 0)
  }

  const changeConcurrency = (value: number): void => {
    writeStoredConcurrency(value)
    concurrencyMutation.mutate(value)
  }

  const configPanel =
    config === undefined ? (
      query.isLoading ? (
        <Skeleton active paragraph={{ rows: 6 }} />
      ) : null
    ) : (
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
            children:
              config === undefined ? null : (
                <Space wrap size={12}>
                  <Switch
                    checked={!config.opencode.paused}
                    loading={concurrencyMutation.isPending}
                    checkedChildren="运行"
                    unCheckedChildren="暂停"
                    onChange={toggleConsumption}
                  />
                  <Select<number>
                    size="small"
                    style={{ minWidth: 88 }}
                    value={config.opencode.concurrency}
                    disabled={config.opencode.paused || concurrencyMutation.isPending}
                    options={CONCURRENCY_OPTIONS.map((v) => ({ value: v, label: `${v} 并发` }))}
                    onChange={changeConcurrency}
                  />
                  <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                    0 = 暂停消费，running 任务自然跑完
                  </Typography.Text>
                </Space>
              ),
          },
          {
            key: 'version',
            label: '后端版本',
            children: (
              <Typography.Text code>{config.version === '' ? '-' : config.version}</Typography.Text>
            ),
          },
        ]}
      />
    )

  return (
    <div>
      <QueryErrorAlert error={query.error} context="加载服务配置" />
      <div className="page-head">
        <Typography.Title level={4} style={{ margin: 0 }}>
          Settings
        </Typography.Title>
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          服务配置 · GET /api/config · 并发调整 PUT /api/config
        </Typography.Text>
      </div>
      <Tabs
        defaultActiveKey="server"
        items={[
          { key: 'server', label: '服务配置', children: configPanel },
          {
            key: 'env',
            label: '环境变量说明',
            children: (
              <div className="flat-panel">
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
                  <Typography.Text code>http://127.0.0.1:7800</Typography.Text>
                  （见 vite.config.ts）。
                </Typography.Paragraph>
              </div>
            ),
          },
        ]}
      />
    </div>
  )
}
