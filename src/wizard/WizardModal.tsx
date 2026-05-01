// SPDX-License-Identifier: BUSL-1.1

import type { ReactNode } from 'react'
import { Heading } from '../components/catalyst/heading'
import { Text } from '../components/catalyst/text'
import { Button } from '../components/catalyst/button'

interface Props {
  title: string
  subtitle: string
  children: ReactNode
  onBack?: () => void
  onSkip?: () => void
  onNext: () => void
  nextLabel?: string
  nextDisabled?: boolean
  helpUrl?: string
}

export default function WizardModal({
  title,
  subtitle,
  children,
  onBack,
  onSkip,
  onNext,
  nextLabel = 'Next',
  nextDisabled = false,
}: Props) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-white dark:bg-zinc-900">
      <div className="w-full max-w-lg px-8 py-10">
        <Heading level={2} className="mb-1">{title}</Heading>
        <Text className="mb-8">{subtitle}</Text>
        <div className="mb-10">{children}</div>
        <div className="flex items-center gap-3">
          {onBack && (
            <Button outline onClick={onBack}>
              Back
            </Button>
          )}
          <Button onClick={onNext} disabled={nextDisabled}>
            {nextLabel}
          </Button>
          {onSkip && (
            <Button plain onClick={onSkip}>
              Skip
            </Button>
          )}
        </div>
      </div>
    </div>
  )
}
