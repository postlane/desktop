// SPDX-License-Identifier: BUSL-1.1

import { openUrl } from '@tauri-apps/plugin-opener'
import WizardModal from './WizardModal'
import { Text } from '../components/catalyst/text'
import { Button } from '../components/catalyst/button'

interface Props {
  onNext: () => void
}

export default function ModalWelcome({ onNext }: Props) {
  async function handlePricingLink() {
    try { await openUrl('https://postlane.dev/pricing') } catch { /* ignore */ }
  }

  return (
    <WizardModal
      title="Welcome to Postlane"
      subtitle="Ship code. Tell the world."
      onNext={onNext}
      nextLabel="Get started"
    >
      <div>
        <Text>This wizard takes about 5 minutes. You&apos;ll:</Text>
        <ul className="mt-3 space-y-1 list-disc list-inside">
          <li>Connect your scheduler</li>
          <li>Link your social profiles</li>
          <li>Connect your first repo</li>
        </ul>
        <Text className="mt-6">
          Free for your first project. $5/month per project after that.
        </Text>
        <Button plain onClick={handlePricingLink} className="mt-2">
          See pricing →
        </Button>
      </div>
    </WizardModal>
  )
}
