import { App as AntdApp } from 'antd'
import { useEffect } from 'react'
import { setMessageInstance } from '../lib/notify'

/** Bind antd's message API once so non-React modules can report errors. */
export function MessageBinder(): null {
  const { message } = AntdApp.useApp()
  useEffect(() => {
    setMessageInstance(message)
  }, [message])
  return null
}
