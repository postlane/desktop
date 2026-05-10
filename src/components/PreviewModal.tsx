// SPDX-License-Identifier: BUSL-1.1

import type { ImageState } from '../types'
import {
  countCharsX,
  countCharsBluesky,
  countCharsMastodon,
  countLinkedInChars,
  countSubstackNotesChars,
} from './charCount'

export const CHAR_LIMITS: Record<string, number> = {
  x: 280,
  bluesky: 300,
  mastodon: 500,
  linkedin: 3000,
  substack_notes: 280,
  substack: 0,
  product_hunt: 260,
  show_hn: 0,
  changelog: 0,
}

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
  const remaining = limit - used
  const isOver = remaining < 0
  return (
    <span className={'is-size-7 ' + (isOver ? 'has-text-danger' : 'has-text-grey')}>
      {remaining}
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
          <span className="tag is-light mr-2">{platform}</span>
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
