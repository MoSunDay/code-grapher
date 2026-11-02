import type { MessageInstance } from 'antd/es/message/interface'

let instance: MessageInstance | null = null

export function setMessageInstance(next: MessageInstance): void {
  instance = next
}

export const notify = {
  error: (content: string): void => {
    instance?.error(content)
  },
  success: (content: string): void => {
    instance?.success(content)
  },
  info: (content: string): void => {
    instance?.info(content)
  },
}
