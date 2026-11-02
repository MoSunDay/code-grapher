import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { WsContext, type WsContextValue, type WsListener } from './ws-context'
import { parseWsEvent, wsEventsUrl } from './ws-events'

export function WsProvider({ children }: { children: ReactNode }) {
  const [connected, setConnected] = useState(false)
  const listenersRef = useRef<Set<WsListener>>(new Set())
  const socketRef = useRef<WebSocket | null>(null)
  const timerRef = useRef<number | null>(null)
  const attemptsRef = useRef(0)
  const stoppedRef = useRef(false)

  useEffect(() => {
    stoppedRef.current = false

    const schedule = (): void => {
      if (stoppedRef.current) return
      const delay = Math.min(10_000, 500 * 2 ** attemptsRef.current)
      attemptsRef.current += 1
      timerRef.current = window.setTimeout(connect, delay)
    }

    const connect = (): void => {
      if (stoppedRef.current) return
      let socket: WebSocket
      try {
        socket = new WebSocket(wsEventsUrl())
      } catch {
        schedule()
        return
      }
      socketRef.current = socket
      socket.onopen = () => {
        attemptsRef.current = 0
        setConnected(true)
      }
      socket.onmessage = (ev: MessageEvent) => {
        let parsed: unknown = null
        try {
          parsed = JSON.parse(typeof ev.data === 'string' ? ev.data : '')
        } catch {
          return
        }
        const event = parseWsEvent(parsed)
        if (event !== null) listenersRef.current.forEach((listener) => listener(event))
      }
      socket.onclose = () => {
        setConnected(false)
        schedule()
      }
      socket.onerror = () => {
        socket.close()
      }
    }

    connect()
    return () => {
      stoppedRef.current = true
      if (timerRef.current !== null) {
        window.clearTimeout(timerRef.current)
        timerRef.current = null
      }
      const socket = socketRef.current
      socketRef.current = null
      if (socket !== null) {
        socket.onclose = null
        socket.close()
      }
      setConnected(false)
    }
  }, [])

  const subscribe = useCallback((listener: WsListener) => {
    listenersRef.current.add(listener)
    return () => {
      listenersRef.current.delete(listener)
    }
  }, [])

  const value = useMemo<WsContextValue>(() => ({ connected, subscribe }), [connected, subscribe])

  return <WsContext.Provider value={value}>{children}</WsContext.Provider>
}
