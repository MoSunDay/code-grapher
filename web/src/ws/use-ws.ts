import { useContext, useEffect, useRef } from 'react'
import { WsContext, type WsContextValue, type WsListener } from './ws-context'

export function useWs(): WsContextValue {
  const ctx = useContext(WsContext)
  if (ctx === null) throw new Error('useWs 必须在 WsProvider 内使用')
  return ctx
}

export function useWsEvent(listener: WsListener): void {
  const { subscribe } = useWs()
  const listenerRef = useRef(listener)
  useEffect(() => {
    listenerRef.current = listener
  }, [listener])
  useEffect(() => {
    const wrapped: WsListener = (event) => listenerRef.current(event)
    return subscribe(wrapped)
  }, [subscribe])
}
