// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback, useEffect, useRef } from 'react'
import { invoke } from '../ipc/invoke'

export interface AsyncListResult<T> {
  data: T[]
  loading: boolean
  error: string | null
  refresh: () => void
  clear: () => void
}

export function useAsyncList<T>(
  command: string,
  args?: Record<string, unknown>,
): AsyncListResult<T> {
  const [data, setData] = useState<T[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const seqRef = useRef(0)
  // Serialised key: load() re-runs when args content changes, not just reference.
  // Parsing inside the callback makes argsKey a real closure dependency (satisfies the
  // exhaustive-deps rule) while avoiding stale-closure issues from capturing args directly.
  const argsKey = args !== undefined ? JSON.stringify(args) : undefined

  const load = useCallback(() => {
    const seq = ++seqRef.current
    setLoading(true)
    setError(null)
    const parsedArgs = argsKey !== undefined
      ? (JSON.parse(argsKey) as Record<string, unknown>)
      : undefined
    invoke<T[]>(command, parsedArgs)
      .then((result) => {
        if (seqRef.current !== seq) return
        setData(Array.isArray(result) ? result : [])
        setLoading(false)
      })
      .catch((e: unknown) => {
        if (seqRef.current !== seq) return
        setError(String(e))
        setLoading(false)
      })
  }, [command, argsKey])

  const refresh = useCallback(() => { load() }, [load])

  const clear = useCallback(() => {
    ++seqRef.current
    setData([])
    setLoading(false)
    setError(null)
  }, [])

  useEffect(() => { load() }, [load])

  return { data, loading, error, refresh, clear }
}
