import { createContext } from 'react'
import type { WsEvent } from '../api/types'

export type WsListener = (event: WsEvent) => void

export interface WsContextValue {
  connected: boolean
  subscribe: (listener: WsListener) => () => void
}

export const WsContext = createContext<WsContextValue | null>(null)
