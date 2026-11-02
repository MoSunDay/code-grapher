import { Alert } from 'antd'
import type { ReactNode } from 'react'
import { extractErrorMessage } from '../api/client'

export function QueryErrorAlert({
  error,
  context,
}: {
  error: unknown
  context?: string
}): ReactNode {
  if (!error) return null
  return (
    <Alert
      type="error"
      showIcon
      style={{ marginBottom: 16 }}
      message={context !== undefined ? `${context}失败` : '请求失败'}
      description={extractErrorMessage(error)}
    />
  )
}
