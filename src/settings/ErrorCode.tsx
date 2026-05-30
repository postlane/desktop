// SPDX-License-Identifier: BUSL-1.1

// Every user-facing error code must display the app version alongside it so
// users and support can correlate the exact build with the error.
// Format: "PL-X-001 · v1.4.2: human readable message"

import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'

interface ErrorCodeProps {
  code: string
  message: string
}

export function ErrorCode({ code, message }: ErrorCodeProps) {
  const [version, setVersion] = useState('')

  useEffect(() => {
    invoke<string>('get_app_version').then(setVersion).catch(() => {})
  }, [])

  const versionSuffix = version ? ` · v${version}` : ''

  return (
    <p className="has-text-danger mb-2">
      <span data-testid="error-code-label">{code}{versionSuffix}</span>: {message}
    </p>
  )
}
