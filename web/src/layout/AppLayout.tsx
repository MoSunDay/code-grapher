import {
  DashboardOutlined,
  FolderOpenOutlined,
  FunctionOutlined,
  SettingOutlined,
  UnorderedListOutlined,
} from '@ant-design/icons'
import { Layout, Menu, Tooltip, Typography } from 'antd'
import { Outlet, useLocation, useNavigate } from 'react-router-dom'
import type { ReactNode } from 'react'
import { useWs } from '../ws/use-ws'
import { RepoSelector } from './RepoSelector'

const NAV_ITEMS = [
  { key: '/', icon: <DashboardOutlined />, label: 'Dashboard' },
  { key: '/files', icon: <FolderOpenOutlined />, label: 'Files' },
  { key: '/functions', icon: <FunctionOutlined />, label: 'Functions' },
  { key: '/tasks', icon: <UnorderedListOutlined />, label: 'Tasks' },
  { key: '/settings', icon: <SettingOutlined />, label: 'Settings' },
]

function WsStatus(): ReactNode {
  const { connected } = useWs()
  return (
    <Tooltip
      title={connected ? 'WebSocket 已连接，实时事件推送中' : 'WebSocket 已断开，自动重连中…'}
    >
      <Typography.Text type={connected ? 'success' : 'secondary'} style={{ fontSize: 12 }}>
        {connected ? '● WS 已连接' : '○ WS 重连中'}
      </Typography.Text>
    </Tooltip>
  )
}

export function AppLayout(): ReactNode {
  const navigate = useNavigate()
  const location = useLocation()
  const selectedKey = `/${location.pathname.split('/')[1] ?? ''}`

  return (
    <Layout style={{ minHeight: '100vh' }}>
      <Layout.Sider width={200} theme="dark">
        <div className="sider-brand">
          <Typography.Title level={5} style={{ color: '#fff', margin: 0 }}>
            code-grapher
          </Typography.Title>
        </div>
        <Menu
          theme="dark"
          mode="inline"
          selectedKeys={[selectedKey]}
          items={NAV_ITEMS}
          onClick={({ key }) => navigate(String(key))}
        />
      </Layout.Sider>
      <Layout>
        <Layout.Header
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            padding: '0 16px',
            background: '#fff',
            borderBottom: '1px solid #f0f0f0',
          }}
        >
          <RepoSelector />
          <WsStatus />
        </Layout.Header>
        <Layout.Content style={{ padding: 16 }}>
          <Outlet />
        </Layout.Content>
      </Layout>
    </Layout>
  )
}
