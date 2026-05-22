// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';

const UNSPLASH_BLOCKED_HOSTNAMES = new Set(['images.unsplash.com', 'plus.unsplash.com']);

function isUnsplashDirectUrl(input: string): boolean {
  try {
    const url = new URL(input);
    return UNSPLASH_BLOCKED_HOSTNAMES.has(url.hostname);
  } catch {
    return false;
  }
}

interface UrlModeProps {
  imageUrl: string | null;
  imageInput: string;
  fetchingOg: boolean;
  ogFetchError: string | null;
  onInputChange: (_v: string) => void;
  onSave: (_url: string) => void;
  onRemove: () => void;
  onCancel: () => void;
}

function UrlMode({ imageUrl, imageInput, fetchingOg, ogFetchError, onInputChange, onSave, onRemove, onCancel }: UrlModeProps) {
  const isUnsplashBlocked = isUnsplashDirectUrl(imageInput);
  const canSave = imageInput.startsWith('https://') && !fetchingOg && !isUnsplashBlocked;
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
      <div className="is-flex is-align-items-center" style={{ gap: '0.5rem' }}>
        <input
          type="url"
          aria-label="Image URL"
          placeholder="https://example.com/image.png or paste a page URL"
          value={imageInput}
          onChange={(e) => onInputChange(e.target.value)}
          className="input is-small"
          style={{ flex: 1 }}
        />
        {imageUrl && (
          <button className="button is-ghost is-small has-text-danger" onClick={onRemove}>Remove</button>
        )}
        <button
          className="button is-small"
          onClick={() => onSave(imageInput)}
          disabled={!canSave}
          aria-label="Save image"
        >
          {fetchingOg ? 'Resolving…' : 'Save image'}
        </button>
        <button className="button is-ghost is-small" onClick={onCancel}>Cancel</button>
      </div>
      {isUnsplashBlocked && (
        <span className="has-text-warning-dark is-size-7">
          Paste is not supported for Unsplash photos — compliance requires selecting via search.
          Use the Search tab.
        </span>
      )}
      {!isUnsplashBlocked && ogFetchError && (
        <span className="has-text-warning-dark is-size-7">{ogFetchError}</span>
      )}
    </div>
  );
}

interface SearchModeProps {
  onSelect: (_url: string, _downloadLocation: string, _attribution: { photographer_name: string; photographer_url: string }) => void;
  onCancel: () => void;
}

function SearchMode({ onSelect: _onSelect, onCancel }: SearchModeProps) {
  const [query, setQuery] = useState('');
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
      <div className="is-flex is-align-items-center" style={{ gap: '0.5rem' }}>
        <input
          type="search"
          aria-label="Search Unsplash"
          placeholder="Search for an image…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          className="input is-small"
          style={{ flex: 1 }}
        />
        <button className="button is-ghost is-small" onClick={onCancel}>Cancel</button>
      </div>
    </div>
  );
}

export interface PostCardImageInputProps {
  imageUrl: string | null;
  imageInput: string;
  fetchingOg: boolean;
  ogFetchError: string | null;
  hasUnsplashKey: boolean;
  onInputChange: (_v: string) => void;
  onSave: (_url: string) => void;
  onRemove: () => void;
  onCancel: () => void;
  onSelectUnsplash: (_url: string, _downloadLocation: string, _attribution: { photographer_name: string; photographer_url: string }) => void;
}

export default function PostCardImageInput({
  imageUrl, imageInput, fetchingOg, ogFetchError, hasUnsplashKey,
  onInputChange, onSave, onRemove, onCancel, onSelectUnsplash,
}: PostCardImageInputProps) {
  const [mode, setMode] = useState<'search' | 'url'>(hasUnsplashKey ? 'search' : 'url');
  return (
    <div className="mt-3" style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
      {hasUnsplashKey && (
        <div className="is-flex" style={{ gap: '0.25rem' }}>
          <button
            className={'button is-small' + (mode === 'search' ? ' is-info' : '')}
            onClick={() => setMode('search')}
          >
            Search
          </button>
          <button
            className={'button is-small' + (mode === 'url' ? ' is-info' : '')}
            onClick={() => setMode('url')}
          >
            URL
          </button>
        </div>
      )}
      {mode === 'url' ? (
        <UrlMode
          imageUrl={imageUrl} imageInput={imageInput} fetchingOg={fetchingOg}
          ogFetchError={ogFetchError} onInputChange={onInputChange}
          onSave={onSave} onRemove={onRemove} onCancel={onCancel}
        />
      ) : (
        <SearchMode onSelect={onSelectUnsplash} onCancel={onCancel} />
      )}
    </div>
  );
}
