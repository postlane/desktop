// SPDX-License-Identifier: BUSL-1.1

import { useEffect } from 'react'
import { useWizardState } from './useWizardState'
import type { WizardState } from './useWizardState'
import ModalWelcome from './ModalWelcome'
import ModalSignIn from './ModalSignIn'
import ModalNameWorkspace from './ModalNameWorkspace'
import ModalConnectScheduler from './ModalConnectScheduler'
import ModalBackupScheduler from './ModalBackupScheduler'
import ModalMapProfiles from './ModalMapProfiles'
import ModalVoiceGuide from './ModalVoiceGuide'
import ModalConnectRepo from './ModalConnectRepo'
import ModalDescribeRepo from './ModalDescribeRepo'
import ModalPlatformOverrides from './ModalPlatformOverrides'
import ModalDone from './ModalDone'
import ModalPricingGate from './ModalPricingGate'

interface Props {
  onComplete: () => void
}

type Actions = ReturnType<typeof useWizardState>

function renderEarlyModals(state: WizardState, actions: Actions) {
  const { next, back, advanceScheduler, setLicenseTokenPresent, setProjectId } = actions
  if (state.modal === 'welcome') return <ModalWelcome onNext={next} />
  if (state.modal === 'sign-in') {
    return <ModalSignIn onNext={next} onBack={back} onTokenDetected={() => setLicenseTokenPresent(true)} />
  }
  if (state.modal === 'name-workspace') {
    return <ModalNameWorkspace onNext={(id) => { setProjectId(id); next() }} onBack={back} />
  }
  if (state.modal === 'connect-scheduler') {
    return <ModalConnectScheduler onNext={next} onBack={back} onSetupLater={advanceScheduler} />
  }
  if (state.modal === 'backup-scheduler') return <ModalBackupScheduler onNext={next} onBack={back} onSkip={actions.skip} />
  if (state.modal === 'map-profiles') return <ModalMapProfiles onNext={next} onBack={back} onSkip={actions.skip} />
  return null
}

const MISSING_PROJECT_ERROR = 'Something went wrong — project ID is missing. Please restart the setup wizard.'

function renderLateModals(state: WizardState, actions: Actions, onComplete: () => void) {
  const { next, back, skip, setSelectedRepoPath, setRepoDescription, advancePastGate } = actions
  const { projectId, selectedRepoPath } = state
  if (state.modal === 'voice-guide') {
    if (!projectId) return <p role="alert">{MISSING_PROJECT_ERROR}</p>
    return <ModalVoiceGuide projectId={projectId} onNext={() => next()} onBack={back} onSkip={skip} />
  }
  if (state.modal === 'connect-repo') {
    if (!projectId) return <p role="alert">{MISSING_PROJECT_ERROR}</p>
    return <ModalConnectRepo
      projectId={projectId}
      onNext={(p) => { setSelectedRepoPath(p); next() }}
      onBack={back}
      onSilentAdd={actions.jumpToDone}
      onPricingGate={actions.jumpToPricingGate}
    />
  }
  if (state.modal === 'describe-repo') {
    if (!projectId) return <p role="alert">{MISSING_PROJECT_ERROR}</p>
    return <ModalDescribeRepo projectId={projectId} repoPath={selectedRepoPath ?? ''} onNext={(d) => { setRepoDescription(d); next() }} onBack={back} />
  }
  if (state.modal === 'platform-overrides') {
    return <ModalPlatformOverrides repoPath={selectedRepoPath ?? ''} onNext={next} onBack={back} onSkip={skip} />
  }
  if (state.modal === 'done') return <ModalDone onComplete={onComplete} />
  if (state.modal === 'pricing-gate') return <ModalPricingGate onPaid={advancePastGate} onBack={back} />
  return null
}

export default function Wizard({ onComplete }: Props) {
  const actions = useWizardState()
  const { state, startFirstLaunch } = actions

  useEffect(() => {
    startFirstLaunch()
  }, [])  // eslint-disable-line react-hooks/exhaustive-deps

  return renderEarlyModals(state, actions) ?? renderLateModals(state, actions, onComplete)
}
