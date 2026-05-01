// SPDX-License-Identifier: BUSL-1.1

import { useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { openUrl } from '@tauri-apps/plugin-opener'
import WizardModal from './WizardModal'
import { Button } from '../components/catalyst/button'
import { Text } from '../components/catalyst/text'

interface Props {
  onPaid: () => void
  onBack: () => void
  pollIntervalMs?: number
  maxAttempts?: number
}

function useBillingPoller(onPaid: () => void, pollIntervalMs: number, maxAttempts: number) {
  const [polling, setPolling] = useState(false)
  const [timedOut, setTimedOut] = useState(false)
  const attemptsRef = useRef(0)
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null)

  function stopPolling() {
    if (intervalRef.current) { clearInterval(intervalRef.current); intervalRef.current = null }
  }

  function begin() {
    setTimedOut(false)
    setPolling(true)
    attemptsRef.current = 0
    intervalRef.current = setInterval(async () => {
      attemptsRef.current++
      try {
        const gate = await invoke<string>('check_billing_gate')
        if (gate === 'paid') { stopPolling(); setPolling(false); onPaid(); return }
      } catch { /* ignore */ }
      if (attemptsRef.current >= maxAttempts) { stopPolling(); setPolling(false); setTimedOut(true) }
    }, pollIntervalMs)
  }

  return { polling, timedOut, begin }
}

export default function ModalPricingGate({
  onPaid, onBack, pollIntervalMs = 5000, maxAttempts = 120,
}: Props) {
  const { polling, timedOut, begin } = useBillingPoller(onPaid, pollIntervalMs, maxAttempts)

  async function handleSubscribe() {
    try { await openUrl('https://postlane.dev/billing') } catch { /* ignore */ }
    begin()
  }

  return (
    <WizardModal
      title="Add a new project"
      subtitle="You've used your free project. Each additional project is $5/month."
      onNext={() => { /* no-op */ }}
      onBack={onBack}
      nextDisabled={true}
    >
      <div className="flex flex-col gap-4">
        <Text>You can invite collaborators to share one subscription across a team.</Text>
        <Button onClick={handleSubscribe} disabled={polling}>Subscribe — $5/month</Button>
        {timedOut && <Button outline onClick={begin}>Check again</Button>}
      </div>
    </WizardModal>
  )
}
