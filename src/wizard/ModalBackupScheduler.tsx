// SPDX-License-Identifier: BUSL-1.1

import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import WizardModal from './WizardModal'
import { Text } from '../components/catalyst/text'

interface Props {
  onNext: () => void
  onBack: () => void
  onSkip: () => void
}

export default function ModalBackupScheduler({ onNext, onBack, onSkip }: Props) {
  const [zernioConfigured, setZernioConfigured] = useState(false)

  useEffect(() => {
    invoke<boolean>('has_provider_credential', { provider: 'zernio' })
      .then((configured) => setZernioConfigured(configured))
      .catch(() => { /* ignore */ })
  }, [])

  return (
    <WizardModal
      title="Add a backup scheduler"
      subtitle="If your primary hits its limit, Postlane falls back automatically."
      onNext={onNext}
      onBack={onBack}
      onSkip={onSkip}
    >
      <div className="flex flex-col gap-4">
        {zernioConfigured && (
          <Text>Zernio is already configured as your primary scheduler.</Text>
        )}
        <Text className="text-sm">
          A backup scheduler is optional. You can always add one later in settings.
        </Text>
      </div>
    </WizardModal>
  )
}
