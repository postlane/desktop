// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { invoke } from '../ipc/invoke';
import { useAsyncCommand } from '../hooks/useAsyncCommand';
import { UnsplashSearch } from '../drafts/PostCardImageInput';
import type { ImageState, ImageAttribution } from '../types';

const UNSPLASH_BLOCKED = new Set(['images.unsplash.com', 'plus.unsplash.com']);

function isUnsplashDirectUrl(url: string): boolean {
  try { return UNSPLASH_BLOCKED.has(new URL(url).hostname); } catch { return false; }
}

function AttributionLine({ attribution }: { attribution: ImageAttribution }) {
  return (
    <p className="is-size-7 has-text-grey mt-1">
      Photo by{' '}
      <a href={attribution.photographer_url} target="_blank" rel="noopener noreferrer">
        {attribution.photographer_name}
      </a>
      {' on '}
      <a href="https://unsplash.com" target="_blank" rel="noopener noreferrer">Unsplash</a>
    </p>
  );
}

export function ImageDisplay({ imageState }: { imageState: ImageState }) {
  if (imageState.status !== 'loaded') return null;
  return (
    <div className="mb-3">
      <img data-testid="og-image" src={imageState.url} alt="Post image"
        style={{ maxWidth: '100%', width: 'auto', height: 'auto', maxHeight: '220px', borderRadius: 4, display: 'block' }} />
      {imageState.attribution && <AttributionLine attribution={imageState.attribution} />}
    </div>
  );
}

export function ImagePickers({ imageState, onCustomSet, onUnsplashSelect, onRemove }: {
  imageState: ImageState;
  onCustomSet: (_url: string) => Promise<void>;
  onUnsplashSelect: (_url: string, _dl: string, _attr: ImageAttribution) => Promise<void>;
  onRemove: () => Promise<void>;
}) {
  const [customUrl, setCustomUrl] = useState('');
  const [validationError, setValidationError] = useState<string | null>(null);
  const { loading: customLoading, error: asyncError, run } = useAsyncCommand();
  const customError = validationError ?? asyncError;
  const [searchClearSignal, setSearchClearSignal] = useState(0);
  const unsplashBlocked = isUnsplashDirectUrl(customUrl);
  async function handleSet() {
    setSearchClearSignal((s) => s + 1);
    setValidationError(null);
    if (unsplashBlocked) return;
    if (!customUrl.startsWith('https://')) { setValidationError('URL must start with https://'); return; }
    await run(async () => {
      await invoke('validate_url_safe', { url: customUrl });
      await onCustomSet(customUrl);
    });
  }
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
      <UnsplashSearch onSelect={onUnsplashSelect} onActivity={() => setValidationError(null)} clearSignal={searchClearSignal} />
      <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
        <p className="is-size-7 has-text-weight-semibold">Add an image from a URL</p>
        <div className="is-flex is-align-items-center" style={{ gap: '0.5rem' }}>
          <input type="url" aria-label="Add an image from a URL" value={customUrl}
            onChange={(e) => { setCustomUrl(e.target.value); setValidationError(null); }} placeholder="https://…"
            className="input is-small" style={{ flex: 1 }} />
          <button className="button is-small is-light" onClick={handleSet} disabled={customLoading || unsplashBlocked}
            data-testid="set-custom-image" style={{ width: '5.5rem' }}>Set image</button>
        </div>
        {unsplashBlocked && (
          <p className="is-size-7 has-text-danger mt-1">
            Use the &quot;Search Unsplash&quot; above to find Unsplash photos.
          </p>
        )}
        {imageState.status === 'loaded' && (
          <div className="mt-2" style={{ display: 'flex', justifyContent: 'flex-end' }}>
            <button className="button is-small has-background-danger has-text-white"
              style={{ border: 'none', width: '5.5rem' }} onClick={onRemove} aria-label="Remove image">
              Remove image
            </button>
          </div>
        )}
      </div>
      {customError && <p role="alert" className="is-size-7 has-text-danger mt-1">{customError}</p>}
    </div>
  );
}

export function ImageSection({ imageState, onCustomSet, onUnsplashSelect, onRemove }: {
  imageState: ImageState;
  onCustomSet: (_url: string) => Promise<void>;
  onUnsplashSelect: (_url: string, _dl: string, _attr: ImageAttribution) => Promise<void>;
  onRemove: () => Promise<void>;
}) {
  return (
    <div className="px-4 py-2" style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
      <ImageDisplay imageState={imageState} />
      <ImagePickers imageState={imageState} onCustomSet={onCustomSet} onUnsplashSelect={onUnsplashSelect} onRemove={onRemove} />
    </div>
  );
}
