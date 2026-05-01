// SPDX-License-Identifier: BUSL-1.1

import { useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { openUrl } from '@tauri-apps/plugin-opener'
import { Heading } from '../components/catalyst/heading'
import { Text } from '../components/catalyst/text'
import { Button } from '../components/catalyst/button'

interface Props {
  onSignedIn: () => void
  pollIntervalMs?: number
}

const SIGN_IN_PROVIDERS = [
  { label: 'GitHub', key: 'github' },
  { label: 'GitLab', key: 'gitlab' },
  { label: 'Google', key: 'google' },
] as const

export default function SignInScreen({ onSignedIn, pollIntervalMs = 2000 }: Props) {
  useEffect(() => {
    const id = setInterval(async () => {
      try {
        const signed = await invoke<boolean>('get_license_signed_in')
        if (signed) {
          clearInterval(id)
          onSignedIn()
        }
      } catch {
        // silently ignore poll errors
      }
    }, pollIntervalMs)
    return () => clearInterval(id)
  }, [onSignedIn, pollIntervalMs])

  function handleProvider(provider: string) {
    openUrl(`https://postlane.dev/login?provider=${provider}`).catch(console.error)
  }

  return (
    <div className="flex h-screen items-center justify-center bg-white dark:bg-zinc-900">
      <div className="flex flex-col items-center gap-6 w-80">
        <div className="flex flex-col items-center gap-2 text-center">
          <Heading level={1}>Sign in to Postlane</Heading>
          <Text>Connect your account to get started.</Text>
        </div>
        <div className="flex flex-col gap-3 w-full">
          {SIGN_IN_PROVIDERS.map(({ label, key }) => (
            <Button key={key} onClick={() => handleProvider(key)} className="w-full">
              {label}
            </Button>
          ))}
        </div>
      </div>
    </div>
  )
}
