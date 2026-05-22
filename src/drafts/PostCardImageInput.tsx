// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback } from 'react';
import { invoke } from '../ipc/invoke';

const UNSPLASH_BLOCKED_HOSTNAMES = new Set(['images.unsplash.com', 'plus.unsplash.com']);

function isUnsplashDirectUrl(input: string): boolean {
  try {
    const url = new URL(input);
    return UNSPLASH_BLOCKED_HOSTNAMES.has(url.hostname);
  } catch {
    return false;
  }
}

interface UnsplashPhoto {
  id: string;
  description: string | null;
  urls: { raw: string; full: string; regular: string; small: string; thumb: string };
  links: { download_location: string };
  user: { name: string; links: { html: string } };
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

type Attribution = { photographer_name: string; photographer_url: string };

interface SearchModeProps {
  onSelect: (_url: string, _downloadLocation: string, _attribution: Attribution) => void;
  onCancel: () => void;
}

interface PhotoGridProps {
  results: UnsplashPhoto[];
  onSelect: SearchModeProps['onSelect'];
}

function PhotoGrid({ results, onSelect }: PhotoGridProps) {
  return (
    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(100px, 1fr))', gap: '0.5rem' }}>
      {results.map((photo) => (
        <button key={photo.id} type="button"
          onClick={() => onSelect(photo.urls.regular, photo.links.download_location, { photographer_name: photo.user.name, photographer_url: photo.user.links.html })}
          className="button is-ghost" style={{ padding: 0, height: 'auto', display: 'block' }}>
          <img src={photo.urls.thumb} alt={photo.description ?? 'Unsplash photo'}
            style={{ width: '100%', borderRadius: '0.25rem', objectFit: 'cover', display: 'block' }} />
          <span className="is-size-7 has-text-grey">{photo.user.name}</span>
        </button>
      ))}
    </div>
  );
}

function SearchMode({ onSelect, onCancel }: SearchModeProps) {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<UnsplashPhoto[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);

  const handleSearch = useCallback(async () => {
    if (!query.trim()) return;
    setSearching(true); setSearchError(null); setResults([]);
    try {
      const photos = await invoke<UnsplashPhoto[]>('search_unsplash', { query: query.trim() });
      setResults(photos);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setSearchError(msg.includes('rate_limit') ? 'rate_limit' : msg);
    } finally { setSearching(false); }
  }, [query]);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
      <div className="is-flex is-align-items-center" style={{ gap: '0.5rem' }}>
        <input type="search" aria-label="Search Unsplash" placeholder="Search for an image…"
          value={query} onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Enter') handleSearch(); }}
          className="input is-small" style={{ flex: 1 }} />
        <button className="button is-small" onClick={handleSearch}
          disabled={!query.trim() || searching} aria-label="Search images">
          {searching ? 'Searching…' : 'Search'}
        </button>
        <button className="button is-ghost is-small" onClick={onCancel}>Cancel</button>
      </div>
      {searchError === 'rate_limit' && (
        <p className="has-text-warning-dark is-size-7">
          Search limit reached — Unsplash free tier allows 50 requests/hour. Try again later.
        </p>
      )}
      {searchError && searchError !== 'rate_limit' && (
        <p className="has-text-warning-dark is-size-7">{searchError}</p>
      )}
      {results.length > 0 && <PhotoGrid results={results} onSelect={onSelect} />}
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
  onSelectUnsplash: (_url: string, _downloadLocation: string, _attribution: Attribution) => void;
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
