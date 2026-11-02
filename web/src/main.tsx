import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { App as AntdApp, ConfigProvider } from 'antd'
import zhCN from 'antd/locale/zh_CN'
import { StrictMode, useEffect } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import App from './App'
import { setMessageInstance } from './lib/notify'
import { RepoProvider } from './repo/RepoProvider'
import { appTheme } from './theme'
import { WsProvider } from './ws/WsProvider'
import './index.css'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 1, staleTime: 15_000, refetchOnWindowFocus: false },
  },
})

function MessageBinder(): null {
  const { message } = AntdApp.useApp()
  useEffect(() => {
    setMessageInstance(message)
  }, [message])
  return null
}

const rootEl = document.getElementById('root')
if (rootEl !== null) {
  createRoot(rootEl).render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <ConfigProvider locale={zhCN} theme={appTheme}>
          <AntdApp>
            <BrowserRouter>
              <RepoProvider>
                <WsProvider>
                  <MessageBinder />
                  <App />
                </WsProvider>
              </RepoProvider>
            </BrowserRouter>
          </AntdApp>
        </ConfigProvider>
      </QueryClientProvider>
    </StrictMode>,
  )
}
