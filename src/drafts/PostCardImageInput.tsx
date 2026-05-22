// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import UnsplashPickerModal, { type UnsplashPhoto, type Attribution } from './UnsplashPickerModal';

const UNSPLASH_BLOCKED_HOSTNAMES = new Set(['images.unsplash.com', 'plus.unsplash.com']);

function isUnsplashDirectUrl(input: string): boolean {
  try {
    return UNSPLASH_BLOCKED_HOSTNAMES.has(new URL(input).hostname);
  } catch {
    return false;
  }
}

interface UnsplashSearchProps {
  onSelect: (_url: string, _dl: string, _attr: Attribution) => void;
  onActivity?: () => void;
  clearSignal?: number;
}

function useUnsplashSearchState(
  onActivity: (() => void) | undefined,
  clearSignal: number | undefined,
) {
  const [query, setQuery] = useState('');
  const [photos, setPhotos] = useState<UnsplashPhoto[]>([]);
  const [page, setPage] = useState(1);
  const [searching, setSearching] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [modalOpen, setModalOpen] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  useEffect(() => {
    if (clearSignal !== undefined) setSearchError(null);
  }, [clearSignal]);
  const handleSearch = useCallback(async () => {
    onActivity?.();
    if (!query.trim()) { setSearchError('empty_query'); return; }
    setSearching(true); setSearchError(null); setPhotos([]); setPage(1);
    try {
      const results = await invoke<UnsplashPhoto[]>('search_unsplash', { query: query.trim(), page: 1 });
      setPhotos(results);
      if (results.length > 0) setModalOpen(true);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setSearchError(msg.includes('rate_limit') ? 'rate_limit' : msg);
    } finally { setSearching(false); }
  }, [query, onActivity]);
  const handleLoadMore = useCallback(async () => {
    const nextPage = page + 1;
    setLoadingMore(true);
    try {
      const more = await invoke<UnsplashPhoto[]>('search_unsplash', { query: query.trim(), page: nextPage });
      setPhotos((prev) => [...prev, ...more]);
      setPage(nextPage);
    } catch { /* non-fatal */ } finally { setLoadingMore(false); }
  }, [page, query]);
  const handleClose = useCallback(() => {
    setModalOpen(false); setPhotos([]); setPage(1);
  }, []);
  return { query, setQuery, photos, searching, loadingMore, modalOpen, searchError, setSearchError, handleSearch, handleLoadMore, handleClose };
}

export function UnsplashSearch({ onSelect, onActivity, clearSignal }: UnsplashSearchProps) {
  const { query, setQuery, photos, searching, loadingMore, modalOpen, searchError, setSearchError, handleSearch, handleLoadMore, handleClose } = useUnsplashSearchState(onActivity, clearSignal);
  return (
    <>
      <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
        <p className="is-size-7 has-text-weight-semibold">Search Unsplash</p>
        <div className="is-flex is-align-items-center" style={{ gap: '0.5rem' }}>
          <input type="search" aria-label="Search Unsplash" placeholder="Search for a photo…"
            value={query} onChange={(e) => { setQuery(e.target.value); setSearchError(null); onActivity?.(); }}
            onKeyDown={(e) => { if (e.key === 'Enter') handleSearch(); }}
            className="input is-small" style={{ flex: 1 }} />
          <button className="button is-small is-light" onClick={handleSearch}
            disabled={searching} aria-label="Search images"
            style={{ width: '5.5rem' }}>
            {searching ? 'Searching…' : 'Search'}
          </button>
        </div>
        {searchError === 'empty_query' && (
          <p role="alert" className="has-text-danger is-size-7">Please enter a search term.</p>
        )}
        {searchError === 'rate_limit' && (
          <p role="alert" className="has-text-danger is-size-7">
            Search limit reached — Unsplash free tier allows 50 requests/hour. Try again later.
          </p>
        )}
        {searchError && searchError !== 'rate_limit' && searchError !== 'empty_query' && (
          <p role="alert" className="has-text-danger is-size-7">{searchError}</p>
        )}
      </div>
      {modalOpen && (
        <UnsplashPickerModal
          photos={photos}
          onSelect={(url, dl, attr) => { onSelect(url, dl, attr); handleClose(); }}
          onClose={handleClose}
          onLoadMore={handleLoadMore}
          loadingMore={loadingMore}
        />
      )}
    </>
  );
}

export interface PostCardImageInputProps {
  imageUrl: string | null;
  imageInput: string;
  fetchingOg: boolean;
  ogFetchError: string | null;
  onInputChange: (_v: string) => void;
  onSave: (_url: string) => void;
  onRemove: () => void;
  onCancel: () => void;
  onSelectUnsplash: (_url: string, _downloadLocation: string, _attribution: Attribution) => void;
}

export default function PostCardImageInput({
  imageUrl, imageInput, fetchingOg, ogFetchError,
  onInputChange, onSave, onRemove, onCancel, onSelectUnsplash,
}: PostCardImageInputProps) {
  const isUnsplashBlocked = isUnsplashDirectUrl(imageInput);
  const canSave = imageInput.startsWith('https://') && !fetchingOg && !isUnsplashBlocked;
  return (
    <div className="mt-3" style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
      <UnsplashSearch onSelect={onSelectUnsplash} />
      <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
        <p className="is-size-7 has-text-weight-semibold">Image URL</p>
        <div className="is-flex is-align-items-center" style={{ gap: '0.5rem' }}>
          <input type="url" aria-label="Image URL"
            placeholder="https://example.com/image.png or paste a page URL"
            value={imageInput} onChange={(e) => onInputChange(e.target.value)}
            className="input is-small" style={{ flex: 1 }} />
          {imageUrl && (
            <button className="button is-ghost is-small has-text-danger" onClick={onRemove}>Remove</button>
          )}
          <button className="button is-small" onClick={() => onSave(imageInput)}
            disabled={!canSave} aria-label="Save image">
            {fetchingOg ? 'Resolving…' : 'Save image'}
          </button>
          <button className="button is-ghost is-small" onClick={onCancel}>Cancel</button>
        </div>
        {isUnsplashBlocked && (
          <span className="has-text-warning-dark is-size-7">
            Paste is not supported for Unsplash photos — use the search above.
          </span>
        )}
        {!isUnsplashBlocked && ogFetchError && (
          <span className="has-text-warning-dark is-size-7">{ogFetchError}</span>
        )}
      </div>
    </div>
  );
}
