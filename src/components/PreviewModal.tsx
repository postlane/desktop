// SPDX-License-Identifier: BUSL-1.1

import type { ImageState } from '../types'
import { PLATFORM_CFG, CHAR_LIMITS } from '../constants/platformConfig'
import {
  countCharsX,
  countCharsBluesky,
  countCharsMastodon,
  countLinkedInChars,
  countSubstackNotesChars,
} from './charCount'

export { CHAR_LIMITS }

const PLATFORM_WIDTH: Record<string, number> = {
  x: 598,
  linkedin: 552,
  bluesky: 600,
  mastodon: 640,
}

function countForPlatform(platform: string, text: string): number {
  if (platform === 'x') return countCharsX(text)
  if (platform === 'bluesky') return countCharsBluesky(text)
  if (platform === 'mastodon') return countCharsMastodon(text)
  if (platform === 'linkedin') return countLinkedInChars(text)
  if (platform === 'substack_notes') return countSubstackNotesChars(text)
  return [...text].length
}

interface CharCountProps {
  platform: string
  text: string
}

export function CharCount({ platform, text }: CharCountProps) {
  const limit = CHAR_LIMITS[platform] ?? 0
  if (limit === 0) return null
  const used = countForPlatform(platform, text)
  const isOver = used > limit
  return (
    <span className={'is-size-7 ' + (isOver ? 'has-text-danger' : 'has-text-grey')}>
      {used} / {limit}
    </span>
  )
}

interface Props {
  platform: string
  text: string
  imageState: ImageState
  onClose: () => void
}

export default function PreviewModal({ platform, text, imageState, onClose }: Props) {
  const width = PLATFORM_WIDTH[platform] ?? 600

  return (
    <div role="dialog" aria-modal="true" className="modal is-active">
      <div className="modal-background" data-testid="modal-overlay" onClick={onClose} />
      <div className="modal-card" style={{ maxWidth: width, width: '100%' }}>
        <header className="modal-card-head">
          <span className="tag is-rounded is-small mr-2"
            style={{ background: PLATFORM_CFG[platform]?.color ?? 'hsl(0,0%,50%)', color: '#fff' }}>
            {PLATFORM_CFG[platform]?.label ?? platform}
          </span>
          <CharCount platform={platform} text={text} />
          <button
            className="delete ml-auto"
            aria-label="Close preview"
            onClick={onClose}
          />
        </header>
        <section className="modal-card-body">
          {imageState.status === 'loaded' && (
            <img src={imageState.url} alt="OG preview" style={{ width: '100%', borderRadius: 4, marginBottom: '0.75rem' }} />
          )}
          <p style={{ whiteSpace: 'pre-wrap' }}>{text}</p>
        </section>
      </div>
    </div>
  )
}
