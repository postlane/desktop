// SPDX-License-Identifier: BUSL-1.1

import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { openUrl } from '@tauri-apps/plugin-opener'
import WizardModal from './WizardModal'
import { Button } from '../components/catalyst/button'
import { Text } from '../components/catalyst/text'

interface Props {
  onNext: () => void
  onTokenDetected: () => void
  onBack?: () => void
  pollIntervalMs?: number
}

const PROVIDERS = ['github', 'gitlab', 'google'] as const
type OAuthProvider = typeof PROVIDERS[number]

function providerLabel(p: OAuthProvider): string {
  return p.charAt(0).toUpperCase() + p.slice(1)
}

function handleOAuth(provider: OAuthProvider) {
  openUrl(`https://postlane.dev/login?provider=${provider}&desktop=1`).catch(() => { /* ignore */ })
}

export default function ModalSignIn({ onNext, onTokenDetected, onBack, pollIntervalMs = 2000 }: Props) {
  const [tokenPresent, setTokenPresent] = useState(false)

  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const signed = await invoke<boolean>('get_license_signed_in')
        if (signed) {
          setTokenPresent(true)
          onTokenDetected()
          clearInterval(interval)
        }
      } catch { /* ignore */ }
    }, pollIntervalMs)
    return () => clearInterval(interval)
  }, [onTokenDetected, pollIntervalMs])

  return (
    <WizardModal
      title="Create your account"
      subtitle="Sign in to activate your free Postlane account."
      onNext={onNext}
      onBack={onBack}
      nextDisabled={!tokenPresent}
    >
      <div className="flex flex-col gap-3">
        {PROVIDERS.map((p) => (
          <Button key={p} outline onClick={() => handleOAuth(p)}>
            {providerLabel(p)}
          </Button>
        ))}
        <Button disabled title="Coming soon">
          Apple
        </Button>
        <Text className="mt-4 text-sm">
          By signing in you agree to our terms and privacy policy.
        </Text>
      </div>
    </WizardModal>
  )
}
