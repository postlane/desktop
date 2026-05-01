// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import WizardModal from './WizardModal'
import { Input } from '../components/catalyst/input'
import { Select } from '../components/catalyst/select'
import { Field, Label } from '../components/catalyst/fieldset'
import { Button } from '../components/catalyst/button'
import { Text } from '../components/catalyst/text'

interface Props {
  onNext: () => void
  onBack: () => void
  onSetupLater: () => void
}

const PROVIDERS = [
  { id: 'zernio', label: 'Zernio (Recommended)' },
  { id: 'publer', label: 'Publer' },
  { id: 'outstand', label: 'Outstand' },
] as const

type ProviderId = typeof PROVIDERS[number]['id']

interface FormProps {
  provider: ProviderId
  apiKey: string
  testing: boolean
  connected: boolean
  testError: string | null
  onProviderChange: (p: ProviderId) => void
  onApiKeyChange: (k: string) => void
  onTest: () => void
  onSetupLater: () => void
}

function SchedulerForm({ provider, apiKey, testing, connected, testError, onProviderChange, onApiKeyChange, onTest, onSetupLater }: FormProps) {
  return (
    <div className="flex flex-col gap-4">
      <Field>
        <Label>Provider</Label>
        <Select value={provider} onChange={(e) => onProviderChange(e.target.value as ProviderId)}>
          {PROVIDERS.map((p) => <option key={p.id} value={p.id}>{p.label}</option>)}
        </Select>
      </Field>
      <Field>
        <Label>API key</Label>
        <Input value={apiKey} onChange={(e) => onApiKeyChange(e.target.value)} placeholder="API key" />
      </Field>
      {testError && <Text className="text-sm text-red-600 dark:text-red-400">{testError}</Text>}
      {connected && <Text className="text-sm text-green-600 dark:text-green-400">Connection successful.</Text>}
      <div className="flex gap-3">
        <Button outline onClick={onTest} disabled={testing || apiKey.trim().length === 0}>Test connection</Button>
        <Button plain onClick={onSetupLater}>Set up later</Button>
      </div>
    </div>
  )
}

export default function ModalConnectScheduler({ onNext, onBack, onSetupLater }: Props) {
  const [provider, setProvider] = useState<ProviderId>('zernio')
  const [apiKey, setApiKey] = useState('')
  const [connected, setConnected] = useState(false)
  const [testError, setTestError] = useState<string | null>(null)
  const [testing, setTesting] = useState(false)

  async function handleTest() {
    setTestError(null)
    setTesting(true)
    try {
      await invoke('test_scheduler', { provider, apiKey })
      setConnected(true)
    } catch (err) {
      setTestError(err instanceof Error ? err.message : String(err))
      setConnected(false)
    } finally {
      setTesting(false)
    }
  }

  return (
    <WizardModal
      title="Connect a scheduler"
      subtitle="Your scheduler publishes to your social accounts. You bring the key."
      onNext={onNext}
      onBack={onBack}
      nextDisabled={!connected}
    >
      <SchedulerForm
        provider={provider} apiKey={apiKey} testing={testing}
        connected={connected} testError={testError}
        onProviderChange={setProvider} onApiKeyChange={setApiKey}
        onTest={handleTest} onSetupLater={onSetupLater}
      />
    </WizardModal>
  )
}
