// SPDX-License-Identifier: BUSL-1.1

import { invoke } from '@tauri-apps/api/core'
import WizardModal from './WizardModal'
import { Text } from '../components/catalyst/text'

interface Props {
  onComplete: () => void
}

export default function ModalDone({ onComplete }: Props) {
  async function handleOpen() {
    try { await invoke('set_wizard_completed') } catch { /* continue regardless */ }
    onComplete()
  }

  return (
    <WizardModal
      title="You're all set"
      subtitle="Draft your first post from your IDE."
      onNext={handleOpen}
      nextLabel="Open Postlane"
    >
      <div className="flex flex-col gap-4">
        <pre className="rounded-md bg-zinc-100 p-4 font-mono text-sm dark:bg-zinc-800">
          /draft-post
        </pre>
        <Text>
          Read the{' '}
          <a
            href="https://docs.postlane.dev/getting-started"
            target="_blank"
            rel="noreferrer"
            className="underline"
          >
            getting started guide
          </a>{' '}
          to learn more.
        </Text>
      </div>
    </WizardModal>
  )
}
