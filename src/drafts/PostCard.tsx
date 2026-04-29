// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback, useEffect, type KeyboardEvent } from 'react';
import { useTimezone, formatTimestamp } from '../TimezoneContext';
import { invoke } from '@tauri-apps/api/core';
import { confirm } from '@tauri-apps/plugin-dialog';
import { ChevronDownIcon } from '@heroicons/react/20/solid';
import { DevicePhoneMobileIcon, ComputerDesktopIcon } from '@heroicons/react/24/outline';
import { Button } from '../components/catalyst/button';
import { Badge } from '../components/catalyst/badge';
import PostPreview from '../components/PostPreview';
import type { DraftPost, Platform, SendResult } from '../types';
import { PLATFORM_LABELS, PLATFORM_ORDER } from '../constants/platforms';

interface Props {
  post: DraftPost;
  onApproved: () => void;
  onDismissed: () => void;
  isFocused?: boolean;
}

function isPlatform(val: unknown): val is Platform {
  return val === 'x' || val === 'bluesky' || val === 'mastodon'
    || val === 'linkedin' || val === 'substack_notes' || val === 'substack'
    || val === 'product_hunt' || val === 'show_hn' || val === 'changelog';
}

const IMAGE_CDN_HOSTNAMES = new Set([
  'images.unsplash.com', 'cdn.pixabay.com', 'images.pexels.com',
  'lh3.googleusercontent.com', 'pbs.twimg.com', 'media.giphy.com',
]);
const IMAGE_EXTENSIONS = ['jpg', 'jpeg', 'png', 'webp', 'gif', 'avif', 'svg'];

function isDirectImageUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    if (IMAGE_CDN_HOSTNAMES.has(parsed.hostname)) return true;
    return IMAGE_EXTENSIONS.some((ext) => parsed.pathname.toLowerCase().endsWith(`.${ext}`));
  } catch { return false; }
}

function triggerText(post: DraftPost): string {
  return post.trigger ? post.trigger : post.post_folder.slice(0, 80);
}

function platformsOnPost(post: DraftPost): Platform[] {
  return PLATFORM_ORDER.filter((p) => post.platforms.includes(p));
}

function PlatformTabs({ platforms, active, onChange }: { platforms: Platform[]; active: Platform; onChange: (_p: Platform) => void }) {
  return (
    <div role="tablist" className="flex gap-1 border-b border-zinc-200 dark:border-zinc-700">
      {platforms.map((p, i) => (
        <button key={p} role="tab" aria-selected={p === active} onClick={() => onChange(p)} data-slot="tab"
          className={['px-3 py-1.5 text-sm font-medium border-b-2 -mb-px focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500', p === active ? 'border-blue-600 text-blue-600 dark:border-blue-400 dark:text-blue-400' : 'border-transparent text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300'].join(' ')}
          aria-label={`${PLATFORM_LABELS[p] ?? p} (${i + 1})`}>
          {PLATFORM_LABELS[p] ?? p}
        </button>
      ))}
    </div>
  );
}

function PlatformResults({ results }: { results: Record<string, string> }) {
  return (
    <div className="flex flex-wrap gap-2">
      {Object.entries(results).map(([platform, result]) => (
        <span key={platform} className="flex items-center gap-1 text-xs">
          <span className="capitalize text-zinc-600 dark:text-zinc-400">{platform}</span>
          {result === 'sent' || result === 'success' ? <span className="text-green-600">✓</span> : <span className="text-red-600">✗</span>}
        </span>
      ))}
    </div>
  );
}

function ViewToggle({ mobileView, onChange }: { mobileView: boolean; onChange: (_m: boolean) => void }) {
  const activeClass = 'rounded bg-zinc-200 p-0.5 text-zinc-900 dark:bg-zinc-600 dark:text-zinc-100';
  const inactiveClass = 'p-0.5 text-zinc-400 dark:text-zinc-500';
  return (
    <div className="flex items-center gap-1 pb-px">
      <Button plain onClick={() => onChange(true)} aria-label="Mobile view" aria-pressed={mobileView} className={mobileView ? activeClass : inactiveClass}><DevicePhoneMobileIcon className="h-4 w-4" /></Button>
      <Button plain onClick={() => onChange(false)} aria-label="Desktop view" aria-pressed={!mobileView} className={!mobileView ? activeClass : inactiveClass}><ComputerDesktopIcon className="h-4 w-4" /></Button>
    </div>
  );
}

function PostCardErrors({ contentLoadError, attributionEnabled, approveError, saveError, retryError }: { contentLoadError: string | null; attributionEnabled: boolean; approveError: string | null; saveError: string | null; retryError: string | null }) {
  return (
    <>
      {contentLoadError && <p className="mt-2 text-xs text-red-600 dark:text-red-400">{contentLoadError}</p>}
      {!attributionEnabled && <p className="text-xs text-amber-600 dark:text-amber-400 mt-1">Attribution footer disabled — toggle in Settings → App</p>}
      {approveError && <p className="mt-2 text-xs text-red-600 dark:text-red-400">{approveError}</p>}
      {saveError && <p className="mt-2 text-xs text-red-600 dark:text-red-400">{saveError}</p>}
      {retryError && <p className="mt-2 text-xs text-red-600 dark:text-red-400">{retryError}</p>}
    </>
  );
}

function PostCardImageInput({ imageUrl, imageInput, fetchingOg, ogFetchError, onInputChange, onSave, onRemove, onCancel }: { imageUrl: string | null; imageInput: string; fetchingOg: boolean; ogFetchError: string | null; onInputChange: (_v: string) => void; onSave: (_url: string) => void; onRemove: () => void; onCancel: () => void }) {
  return (
    <div className="mt-3 flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <input type="url" aria-label="Image URL" placeholder="https://example.com/image.png or paste a page URL" value={imageInput} onChange={(e) => onInputChange(e.target.value)} className="flex-1 rounded border border-zinc-300 px-2 py-1 text-sm dark:border-zinc-600 dark:bg-zinc-800 dark:text-zinc-100 focus:outline-none focus:ring-2 focus:ring-blue-500" />
        {imageUrl && <Button plain onClick={onRemove} className="text-rose-700 dark:text-rose-400">Remove</Button>}
        <Button onClick={() => onSave(imageInput)} disabled={!imageInput.startsWith('https://') || fetchingOg} aria-label="Save image">{fetchingOg ? 'Resolving…' : 'Save image'}</Button>
        <Button plain onClick={onCancel}>Cancel</Button>
      </div>
      {ogFetchError && <span className="text-xs text-amber-600 dark:text-amber-400">{ogFetchError}</span>}
    </div>
  );
}

function PostCardRedraft({ post, redraftInstruction, redraftQueued, redraftError, onInstructionChange, onQueue, onCancel }: { post: DraftPost; redraftInstruction: string; redraftQueued: boolean; redraftError: string | null; onInstructionChange: (_v: string) => void; onQueue: () => void; onCancel: () => void }) {
  return (
    <div className="mt-4 flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <input type="search" placeholder="Ask the LLM to revise… e.g. 'make it shorter'" aria-label="Redraft instruction" value={redraftInstruction} onChange={(e) => onInstructionChange(e.target.value)} maxLength={10000} className="flex-1 rounded-lg border border-zinc-300 bg-white px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 dark:text-zinc-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500" />
        <Button outline disabled={!redraftInstruction.trim()} onClick={onQueue}>Queue for redraft</Button>
      </div>
      {redraftError && <p className="mt-2 text-xs text-red-600 dark:text-red-400">{redraftError}</p>}
      {redraftQueued && <p className="mt-2 text-xs text-zinc-500 dark:text-zinc-400">Queued for redraft — open your IDE and run <code>/redraft-post</code>.</p>}
      {redraftQueued && <button type="button" onClick={onCancel} className="text-sm text-gray-500 underline hover:text-gray-700">Cancel redraft</button>}
      {/* post is used for redraft context */}
      <span data-post-folder={post.post_folder} className="hidden" />
    </div>
  );
}

function usePostCardContent(post: DraftPost, activeTab: Platform) {
  const [postContent, setPostContent] = useState<string>('');
  const [contentLoadError, setContentLoadError] = useState<string | null>(null);
  const [attributionEnabled, setAttributionEnabled] = useState(true);

  useEffect(() => {
    invoke<string>('get_post_content', { repoPath: post.repo_path, postFolder: post.post_folder, platform: activeTab })
      .then((c) => { setPostContent(typeof c === 'string' ? c : ''); setContentLoadError(null); })
      .catch((e) => { setPostContent(''); setContentLoadError('Failed to load post content.'); console.error('get_post_content failed:', e); });
  }, [activeTab, post.repo_path, post.post_folder]);

  useEffect(() => { invoke<boolean>('get_attribution').then((v) => setAttributionEnabled(v)).catch(() => {}); }, []);

  return { postContent, setPostContent, contentLoadError, attributionEnabled };
}

function usePostCardActions(post: DraftPost, onApproved: () => void, onDismissed: () => void) {
  const [approving, setApproving] = useState(false);
  const [approveError, setApproveError] = useState<string | null>(null);
  const [fallbackNotice, setFallbackNotice] = useState<string | null>(null);
  const [retrying, setRetrying] = useState(false);
  const [retryError, setRetryError] = useState<string | null>(null);

  const approve = useCallback(async () => {
    setApproving(true); setApproveError(null);
    try {
      const result = await invoke<SendResult>('approve_post', { repoPath: post.repo_path, postFolder: post.post_folder });
      if (result.fallback_provider) { setFallbackNotice(result.fallback_provider); } else { onApproved(); }
    }
    catch (e) { setApproveError(e instanceof Error ? e.message : String(e)); }
    finally { setApproving(false); }
  }, [post, onApproved]);

  const dismissFallbackNotice = useCallback(() => { setFallbackNotice(null); onApproved(); }, [onApproved]);

  const dismiss = useCallback(async () => {
    const yes = await confirm('Delete this post? This cannot be undone.', { title: 'Delete post', kind: 'warning' });
    if (!yes) return;
    try { await invoke('delete_post', { repoPath: post.repo_path, postFolder: post.post_folder }); onDismissed(); }
    catch (e) { console.error('delete_post failed:', e); }
  }, [post, onDismissed]);

  const retry = useCallback(async () => {
    setRetrying(true); setRetryError(null);
    try { await invoke('retry_post', { repoPath: post.repo_path, postFolder: post.post_folder }); onApproved(); }
    catch (e) { setRetryError(e instanceof Error ? e.message : String(e)); }
    finally { setRetrying(false); }
  }, [post, onApproved]);

  return { approving, approveError, fallbackNotice, dismissFallbackNotice, retrying, retryError, approve, dismiss, retry };
}

function usePostCardRedraft(post: DraftPost) {
  const [redraftInstruction, setRedraftInstruction] = useState('');
  const [redraftQueued, setRedraftQueued] = useState(false);
  const [redraftError, setRedraftError] = useState<string | null>(null);

  const handleQueueRedraft = useCallback(async () => {
    const ok = await confirm(`Queue for redraft with instruction: "${redraftInstruction.trim()}"?`, { title: 'Confirm redraft', kind: 'info' });
    if (!ok) return;
    try { await invoke('queue_redraft', { repoPath: post.repo_path, postFolder: post.post_folder, instruction: redraftInstruction.trim() }); setRedraftQueued(true); setRedraftError(null); }
    catch (e) { setRedraftError(e instanceof Error ? e.message : String(e)); }
  }, [post, redraftInstruction]);

  const handleCancelRedraft = useCallback(async () => {
    try { await invoke('cancel_redraft', { repoPath: post.repo_path }); setRedraftQueued(false); setRedraftInstruction(''); setRedraftError(null); }
    catch (e) { console.error('cancel_redraft failed:', e); }
  }, [post]);

  const handleInstructionChange = useCallback((v: string) => { setRedraftInstruction(v); setRedraftQueued(false); setRedraftError(null); }, []);
  return { redraftInstruction, redraftQueued, redraftError, handleQueueRedraft, handleCancelRedraft, handleInstructionChange };
}

function useMastodonCharLimit(activeTab: Platform) {
  const [charLimit, setCharLimit] = useState<number | undefined>(undefined);
  useEffect(() => {
    if (activeTab !== 'mastodon') { setCharLimit(undefined); return; }
    invoke<string | null>('get_mastodon_connected_instance')
      .then((instance) => {
        if (!instance) return;
        return invoke<number>('get_mastodon_char_limit', { instance });
      })
      .then((limit) => { if (limit !== undefined) setCharLimit(limit); })
      .catch(() => {});
  }, [activeTab]);
  return charLimit;
}

function PostCardBody({ post, platforms, activeTab, isFailed, approving, approveError, retrying, retryError, onApprove, onDelete, onTabChange }: { post: DraftPost; platforms: Platform[]; activeTab: Platform; isFailed: boolean; approving: boolean; approveError: string | null; retrying: boolean; retryError: string | null; onApprove: () => void; onDelete: () => void; onTabChange: (_p: Platform) => void }) {
  const [mobileView, setMobileView] = useState(true);
  const [imageUrl, setImageUrl] = useState<string | null>(post.image_url ?? null);
  const [addingImage, setAddingImage] = useState(false);
  const [imageInput, setImageInput] = useState('');
  const [fetchingOg, setFetchingOg] = useState(false);
  const [ogFetchError, setOgFetchError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const { postContent, setPostContent, contentLoadError, attributionEnabled } = usePostCardContent(post, activeTab);
  const { redraftInstruction, redraftQueued, redraftError, handleQueueRedraft, handleCancelRedraft, handleInstructionChange } = usePostCardRedraft(post);
  const mastodonCharLimit = useMastodonCharLimit(activeTab);

  const handleSave = useCallback(async (newContent: string) => {
    try { await invoke('update_post_content', { repoPath: post.repo_path, postFolder: post.post_folder, platform: activeTab, newContent }); setPostContent(newContent); setSaveError(null); }
    catch (e) { setSaveError('Failed to save changes. Try again.'); console.error('update_post_content failed:', e); }
  }, [post, activeTab, setPostContent]);

  const handleSaveImage = useCallback(async (url: string) => {
    let resolvedUrl = url;
    if (!isDirectImageUrl(url)) {
      setFetchingOg(true); setOgFetchError(null);
      try { const found = await invoke<string | null>('fetch_og_image', { url }); if (found) { resolvedUrl = found; } else { setOgFetchError('No image found on that page. Paste a direct image URL instead.'); setFetchingOg(false); return; } }
      catch (e) { setOgFetchError(e instanceof Error ? e.message : String(e)); setFetchingOg(false); return; }
      setFetchingOg(false);
    }
    try { await invoke('update_post_image', { repoPath: post.repo_path, postFolder: post.post_folder, imageUrl: resolvedUrl }); setImageUrl(resolvedUrl); setAddingImage(false); setImageInput(''); setOgFetchError(null); }
    catch (e) { console.error('update_post_image failed:', e); }
  }, [post]);

  const handleRemoveImage = useCallback(async () => {
    try { await invoke('update_post_image', { repoPath: post.repo_path, postFolder: post.post_folder, imageUrl: null }); setImageUrl(null); }
    catch (e) { console.error('update_post_image failed:', e); }
  }, [post]);

  const approveLabel = isFailed ? (retrying ? 'Retrying…' : 'Retry') : (approving ? 'Approving…' : 'Approve');

  return (
    <div className="mt-4">
      <div className="flex items-center justify-between">
        <PlatformTabs platforms={platforms} active={activeTab} onChange={onTabChange} />
        <ViewToggle mobileView={mobileView} onChange={setMobileView} />
      </div>
      <div data-testid="preview-container" className={`mt-3 rounded-lg bg-zinc-50 p-3 dark:bg-zinc-800 ${mobileView ? 'max-w-[375px]' : 'max-w-[600px]'}`}>
        <PostPreview content={postContent} platform={activeTab} imageUrl={imageUrl ?? undefined}
          charLimit={mastodonCharLimit} onSave={handleSave}
          onImageClick={() => { setImageInput(imageUrl ?? ''); setAddingImage(true); setOgFetchError(null); }}
          onApprove={onApprove} approveLabel={approveLabel} onDelete={onDelete} />
      </div>
      {addingImage && <PostCardImageInput imageUrl={imageUrl} imageInput={imageInput} fetchingOg={fetchingOg} ogFetchError={ogFetchError} onInputChange={(v) => { setImageInput(v); setOgFetchError(null); }} onSave={handleSaveImage} onRemove={handleRemoveImage} onCancel={() => { setAddingImage(false); setImageInput(''); setOgFetchError(null); }} />}
      <PostCardErrors contentLoadError={contentLoadError} attributionEnabled={attributionEnabled} approveError={approveError} saveError={saveError} retryError={retryError} />
      <PostCardRedraft post={post} redraftInstruction={redraftInstruction} redraftQueued={redraftQueued} redraftError={redraftError} onInstructionChange={handleInstructionChange} onQueue={handleQueueRedraft} onCancel={handleCancelRedraft} />
    </div>
  );
}

function FailedErrorBanner({ error, platformResults }: { error: string | null; platformResults: Record<string, string> | null }) {
  if (!error) return null;
  return (
    <div className="mt-3 rounded-lg bg-red-50 p-3 dark:bg-red-900/20">
      <p className="text-xs text-red-700 dark:text-red-400">{error}</p>
      {platformResults && <div className="mt-2"><PlatformResults results={platformResults} /></div>}
    </div>
  );
}

function usePostCardKeyboard(
  isFocused: boolean, isFailed: boolean, platforms: Platform[],
  approve: () => void, dismiss: () => void, retry: () => void,
  setActiveTab: (_p: Platform) => void, setExpanded: (_fn: (_v: boolean) => boolean) => void,
) {
  return useCallback((e: KeyboardEvent<HTMLElement>) => {
    if (!isFocused) return;
    const key = e.key.toLowerCase();
    const numIdx = parseInt(key, 10) - 1;
    if (numIdx >= 0 && numIdx < Math.min(5, platforms.length)) { setActiveTab(platforms[numIdx]); return; }
    const actions: Partial<Record<string, () => void>> = {
      a: () => { e.preventDefault(); approve(); },
      d: () => { e.preventDefault(); dismiss(); },
      e: () => { e.preventDefault(); setExpanded((v) => !v); },
      r: () => { if (isFailed) { e.preventDefault(); retry(); } },
      escape: () => { e.preventDefault(); setExpanded(() => false); },
    };
    actions[key]?.();
  }, [isFocused, isFailed, platforms, approve, dismiss, retry, setActiveTab, setExpanded]);
}

function FallbackNotice({ provider, onDismiss }: { provider: string; onDismiss: () => void }) {
  return (
    <div role="alert" className="mt-3 rounded-lg bg-amber-50 px-3 py-2 text-xs text-amber-800 dark:bg-amber-900/20 dark:text-amber-300">
      Posted via {provider} — your primary provider has reached its limit.{' '}
      <button type="button" onClick={onDismiss} className="font-medium underline">Got it</button>
    </div>
  );
}

export default function PostCard({ post, onApproved, onDismissed, isFocused = false }: Props) {
  const tz = useTimezone();
  const isFailed = post.status === 'failed';
  const [expanded, setExpanded] = useState(isFailed);
  const [activeTab, setActiveTab] = useState<Platform>(isPlatform(post.platforms[0]) ? post.platforms[0] : 'x');
  const platforms = platformsOnPost(post);
  const { approving, approveError, fallbackNotice, dismissFallbackNotice, retrying, retryError, approve, dismiss, retry } = usePostCardActions(post, onApproved, onDismissed);
  const focusClass = isFocused ? 'ring-2 ring-blue-500 bg-blue-50/40 dark:bg-blue-900/10' : '';
  const handleKeyDown = usePostCardKeyboard(isFocused, isFailed, platforms, approve, dismiss, retry, setActiveTab, setExpanded);

  return (
    <article role="article" data-post-card onKeyDown={handleKeyDown} tabIndex={0} className={`rounded-xl border border-zinc-200 bg-white p-4 dark:border-zinc-700 dark:bg-zinc-900 focus:outline-none ${focusClass}`}>
      <div className="flex items-start gap-3">
        <div className="min-w-0 flex-1">
          <div className="mb-1 flex items-center gap-2">
            <Badge color="zinc">{post.repo_name}</Badge>
            {isFailed && <Badge color="red">Failed</Badge>}
          </div>
          <p className="truncate text-sm font-medium text-zinc-900 dark:text-zinc-100">{triggerText(post)}</p>
          <p className="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">
            {post.platforms.join(' · ')}
            {post.schedule && <> · {formatTimestamp(post.schedule, tz)}</>}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <Button color="blue" onClick={() => setExpanded((v) => !v)} aria-label="Preview" aria-expanded={expanded}>
            <ChevronDownIcon className={`h-4 w-4 transition-transform ${expanded ? 'rotate-180' : ''}`} />
            Preview
          </Button>
        </div>
      </div>
      {isFailed && <FailedErrorBanner error={post.error} platformResults={post.platform_results} />}
      {fallbackNotice && <FallbackNotice provider={fallbackNotice} onDismiss={dismissFallbackNotice} />}
      {expanded && platforms.length > 0 && (
        <PostCardBody post={post} platforms={platforms} activeTab={activeTab} isFailed={isFailed} approving={approving} approveError={approveError} retrying={retrying} retryError={retryError} onApprove={approve} onDelete={dismiss} onTabChange={setActiveTab} />
      )}
    </article>
  );
}
