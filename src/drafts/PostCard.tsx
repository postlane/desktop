// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback } from 'react';
import { useTimezone, formatTimestamp, getTimezoneOffsetLabel } from '../TimezoneContext';
import { invoke } from '../ipc/invoke';
import { openUrl } from '@tauri-apps/plugin-opener';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faChevronDown, faMobileScreen, faDesktop } from '@fortawesome/free-solid-svg-icons';
import PostPreview from '../components/PostPreview';
import type { DraftPost, Platform } from '../types';
import { PLATFORM_LABELS, PLATFORM_ORDER } from '../constants/platforms';
import { usePostCardContent } from '../hooks/usePostCardContent';
import { usePostCardActions, type ApproveBlockedInfo } from '../hooks/usePostCardActions';
import { usePostCardRedraft } from '../hooks/usePostCardRedraft';
import { useMastodonCharLimit } from '../hooks/useMastodonCharLimit';
import { usePostCardKeyboard } from '../hooks/usePostCardKeyboard';
import { ScheduleRow } from './ScheduleRow';
import PostCardImageInput from './PostCardImageInput';
import { usePostCardImage } from '../hooks/usePostCardImage';
import { SendSuccessModal } from '../components/SendSuccessModal';

interface Props {
  post: DraftPost;
  onApproved: () => void;
  onDismissed: () => void;
  isFocused?: boolean;
  /** Platform slugs with a working connection. Undefined = not yet loaded (show all). */
  connectedPlatforms?: string[];
  /** Whether the org has an Unsplash API key configured. Checked once at the queue level. */
  hasUnsplashKey?: boolean;
}

function isPlatform(val: unknown): val is Platform {
  return val === 'x' || val === 'bluesky' || val === 'mastodon'
    || val === 'linkedin' || val === 'substack_notes' || val === 'substack'
    || val === 'product_hunt' || val === 'show_hn' || val === 'changelog';
}


function triggerText(post: DraftPost): string {
  return post.trigger ? post.trigger : post.post_folder.slice(0, 80);
}

function platformsOnPost(post: DraftPost): Platform[] {
  return PLATFORM_ORDER.filter((p) => post.platforms.includes(p));
}

function PlatformTabs({ platforms, active, onChange }: { platforms: Platform[]; active: Platform; onChange: (_p: Platform) => void }) {
  return (
    <div role="tablist" className="tabs is-boxed is-small">
      {platforms.map((p, i) => (
        <a key={p} role="tab" aria-selected={p === active} onClick={() => onChange(p)}
          className={'tab' + (p === active ? ' is-active' : '')}
          aria-label={`${PLATFORM_LABELS[p] ?? p} (${i + 1})`}>
          {PLATFORM_LABELS[p] ?? p}
        </a>
      ))}
    </div>
  );
}

function PlatformResults({ results }: { results: Record<string, string> }) {
  return (
    <div className="tags">
      {Object.entries(results).map(([platform, result]) => (
        <span key={platform} className="is-flex is-align-items-center is-size-7" style={{ gap: '0.25rem' }}>
          <span className="is-capitalized has-text-grey">{platform}</span>
          {result === 'sent' || result === 'success' ? <span className="has-text-success">✓</span> : <span className="has-text-danger">✗</span>}
        </span>
      ))}
    </div>
  );
}

function ViewToggle({ mobileView, onChange }: { mobileView: boolean; onChange: (_m: boolean) => void }) {
  return (
    <div className="is-flex is-align-items-center" style={{ gap: '0.25rem' }}>
      <button className={'button is-ghost is-small' + (mobileView ? ' has-background-grey-lighter' : '')} onClick={() => onChange(true)} aria-label="Mobile view" aria-pressed={mobileView}>
        <FontAwesomeIcon icon={faMobileScreen} />
      </button>
      <button className={'button is-ghost is-small' + (!mobileView ? ' has-background-grey-lighter' : '')} onClick={() => onChange(false)} aria-label="Desktop view" aria-pressed={!mobileView}>
        <FontAwesomeIcon icon={faDesktop} />
      </button>
    </div>
  );
}

function PostCardErrors({ contentLoadError, attributionEnabled, approveError, saveError, retryError, dismissError, imageSaveError }: { contentLoadError: string | null; attributionEnabled: boolean; approveError: string | null; saveError: string | null; retryError: string | null; dismissError: string | null; imageSaveError: string | null }) {
  return (
    <>
      {contentLoadError && <p className="has-text-danger is-size-7 mt-2">{contentLoadError}</p>}
      {!attributionEnabled && <p className="has-text-warning-dark is-size-7 mt-1">Attribution footer disabled — toggle in Settings → App</p>}
      {approveError && <p className="has-text-danger is-size-7 mt-2">{approveError}</p>}
      {saveError && <p className="has-text-danger is-size-7 mt-2">{saveError}</p>}
      {retryError && <p className="has-text-danger is-size-7 mt-2">{retryError}</p>}
      {dismissError && <p role="alert" className="has-text-danger is-size-7 mt-2">{dismissError}</p>}
      {imageSaveError && <p role="alert" className="has-text-danger is-size-7 mt-2">{imageSaveError}</p>}
    </>
  );
}


function PostCardRedraft({ post, redraftInstruction, redraftQueued, redraftError, cancelRedraftError, onInstructionChange, onQueue, onCancel }: { post: DraftPost; redraftInstruction: string; redraftQueued: boolean; redraftError: string | null; cancelRedraftError: string | null; onInstructionChange: (_v: string) => void; onQueue: () => void; onCancel: () => void }) {
  return (
    <div className="mt-4" style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
      <div className="is-flex is-align-items-center" style={{ gap: '0.5rem' }}>
        <input type="search" placeholder="Ask the LLM to revise… e.g. 'make it shorter'" aria-label="Redraft instruction" value={redraftInstruction} onChange={(e) => onInstructionChange(e.target.value)} maxLength={10000} className="input is-small" style={{ flex: 1 }} />
        <button className="button is-outlined is-small" disabled={!redraftInstruction.trim()} onClick={onQueue}>Queue for redraft</button>
      </div>
      {redraftError && <p className="has-text-danger is-size-7 mt-2">{redraftError}</p>}
      {cancelRedraftError && <p role="alert" className="has-text-danger is-size-7 mt-2">{cancelRedraftError}</p>}
      {redraftQueued && <p className="has-text-grey is-size-7 mt-2">Queued for redraft — open your IDE and run <code>/redraft-post</code>.</p>}
      {redraftQueued && <button type="button" onClick={onCancel} className="button is-ghost is-small">Cancel redraft</button>}
      <span data-post-folder={post.post_folder} style={{ display: 'none' }} />
    </div>
  );
}

function PostCardBody({ post, platforms, activeTab, isFailed, approving, approveError, approveBlockedInfo, retrying, retryError, dismissError, schedule, hasUnsplashKey: _hasUnsplashKey, onApprove, onDelete, onTabChange, onScheduleChange }: { post: DraftPost; platforms: Platform[]; activeTab: Platform; isFailed: boolean; approving: boolean; approveError: string | null; approveBlockedInfo: ApproveBlockedInfo | null; retrying: boolean; retryError: string | null; dismissError: string | null; schedule: string | null; hasUnsplashKey?: boolean; onApprove: () => void; onDelete: () => void; onTabChange: (_p: Platform) => void; onScheduleChange: (_s: string | null) => void }) {
  const [mobileView, setMobileView] = useState(true);
  const [saveError, setSaveError] = useState<string | null>(null);
  const { imageUrl, addingImage, imageInput, fetchingOg, ogFetchError, imageSaveError,
    openImageInput, closeImageInput, handleSaveImage, handleRemoveImage, handleSelectUnsplash, onInputChange } = usePostCardImage(post);
  const { postContent, setPostContent, contentLoadError, attributionEnabled } = usePostCardContent(post, activeTab);
  const { redraftInstruction, redraftQueued, redraftError, cancelRedraftError, handleQueueRedraft, handleCancelRedraft, handleInstructionChange } = usePostCardRedraft(post);
  const mastodonCharLimit = useMastodonCharLimit(activeTab);
  const approveLabel = isFailed ? (retrying ? 'Retrying…' : 'Retry') : (approving ? 'Approving…' : 'Approve');

  const handleSave = useCallback(async (newContent: string) => {
    try { await invoke('update_post_content', { repoPath: post.repo_path, postFolder: post.post_folder, platform: activeTab, newContent }); setPostContent(newContent); setSaveError(null); }
    catch (e) { setSaveError('Failed to save changes. Try again.'); console.error('update_post_content failed:', e); }
  }, [post, activeTab, setPostContent]);

  return (
    <div className="mt-4">
      <div className="is-flex is-align-items-center is-justify-content-space-between">
        <PlatformTabs platforms={platforms} active={activeTab} onChange={onTabChange} />
        <ViewToggle mobileView={mobileView} onChange={setMobileView} />
      </div>
      <div data-testid="preview-container" data-mobile={mobileView ? 'true' : 'false'} className="mt-3" style={{ maxWidth: mobileView ? 375 : 600, background: 'var(--bulma-scheme-main-bis)', borderRadius: '0.5rem', padding: '0.75rem' }}>
        <PostPreview content={postContent} platform={activeTab} imageUrl={imageUrl ?? undefined}
          charLimit={mastodonCharLimit} onSave={handleSave}
          onImageClick={openImageInput} onApprove={onApprove} approveLabel={approveLabel} onDelete={onDelete} />
      </div>
      {addingImage && (
        <PostCardImageInput
          imageUrl={imageUrl} imageInput={imageInput} fetchingOg={fetchingOg}
          ogFetchError={ogFetchError}
          onInputChange={onInputChange} onSave={handleSaveImage} onRemove={handleRemoveImage}
          onCancel={closeImageInput}
          onSelectUnsplash={(url, dl, attr) => { handleSelectUnsplash(url, dl, attr); closeImageInput(); }}
        />
      )}
      {post.image_attribution && (
        <p className="is-size-7 has-text-grey mt-2">
          Photo by{' '}
          <a href={post.image_attribution.photographer_url} target="_blank" rel="noopener noreferrer">
            {post.image_attribution.photographer_name}
          </a>
          {' '}on Unsplash
        </p>
      )}
      <ScheduleRow repoPath={post.repo_path} postFolder={post.post_folder} schedule={schedule} onScheduleChange={onScheduleChange} />
      {approveBlockedInfo && <ApprovalBlockedBanner info={approveBlockedInfo} />}
      <PostCardErrors contentLoadError={contentLoadError} attributionEnabled={attributionEnabled} approveError={approveError} saveError={saveError} retryError={retryError} dismissError={dismissError} imageSaveError={imageSaveError} />
      <PostCardRedraft post={post} redraftInstruction={redraftInstruction} redraftQueued={redraftQueued} redraftError={redraftError} cancelRedraftError={cancelRedraftError} onInstructionChange={handleInstructionChange} onQueue={handleQueueRedraft} onCancel={handleCancelRedraft} />
    </div>
  );
}

function PostCardMeta({ post, localSchedule, connectedPlatforms }: { post: DraftPost; localSchedule: string | null; connectedPlatforms: string[] | undefined }) {
  const tz = useTimezone();
  const visible = connectedPlatforms === undefined
    ? post.platforms
    : post.platforms.filter((p) => connectedPlatforms.includes(p));
  return (
    <p data-testid="platform-meta" className="has-text-grey is-size-7 mt-1">
      {visible.length > 0
        ? visible.join(' · ')
        : connectedPlatforms !== undefined
          ? <span className="has-text-grey-light">Connect a platform to approve this post</span>
          : null}
      {localSchedule && <> · {formatTimestamp(localSchedule, tz)} {getTimezoneOffsetLabel(tz)}</>}
      {post.schedule_source === 'default' && <> · <span className="tag is-light is-size-7">auto</span></>}
      {post.llm_model && <> · <span className="tag is-light is-size-7">{post.llm_model}</span></>}
    </p>
  );
}

function FailedErrorBanner({ error, platformResults }: { error: string | null; platformResults: Record<string, string> | null }) {
  if (!error) return null;
  return (
    <div className="notification is-danger is-light mt-3" style={{ padding: '0.75rem' }}>
      <p className="is-size-7">{error}</p>
      {platformResults && <div className="mt-2"><PlatformResults results={platformResults} /></div>}
    </div>
  );
}

// checklist 24.4.11 — CTA copy differs by license_status and by whether the
// caller owns the workspace or collaborates on it. payment_failed
// (collaborator) and unlicensed (either role) share the take-over-billing
// framing, since an admin collaborator can already initiate transfer during
// payment_failed's grace period, not just once it lapses into unlicensed.
function ApprovalBlockedBanner({ info }: { info: ApproveBlockedInfo }) {
  const { status, isOwner, daysRemaining } = info;
  const openDashboard = () => openUrl('https://postlane.dev/dashboard');

  if (status === 'inactive' && isOwner) {
    return (
      <div role="alert" className="notification is-warning is-light mt-3 is-size-7" style={{ padding: '0.5rem 0.75rem' }}>
        <p>This workspace is paused.</p>
        <button type="button" className="button is-small is-warning is-light mt-2" onClick={openDashboard}>
          Reactivate to resume posting
        </button>
      </div>
    );
  }
  if (status === 'inactive') {
    return (
      <div role="alert" className="notification is-light mt-3 is-size-7" style={{ padding: '0.5rem 0.75rem' }}>
        This workspace is paused by its owner.
      </div>
    );
  }
  if (status === 'payment_failed' && isOwner) {
    const days = daysRemaining ?? 0;
    return (
      <div role="alert" className="notification is-danger is-light mt-3 is-size-7" style={{ padding: '0.5rem 0.75rem' }}>
        <p>You have {days} day{days === 1 ? '' : 's'} left before an admin collaborator can take over billing.</p>
        <button type="button" className="button is-small is-danger is-light mt-2" onClick={openDashboard}>
          Update billing
        </button>
      </div>
    );
  }
  return (
    <div role="alert" className="notification is-danger is-light mt-3 is-size-7" style={{ padding: '0.5rem 0.75rem' }}>
      <p>Take over billing or continue read-only.</p>
      <button type="button" className="button is-small is-danger is-light mt-2" onClick={openDashboard}>
        Manage billing
      </button>
    </div>
  );
}

function FallbackNotice({ provider, onDismiss }: { provider: string; onDismiss: () => void }) {
  return (
    <div role="alert" className="notification is-warning is-light mt-3 is-size-7" style={{ padding: '0.5rem 0.75rem' }}>
      Posted via {provider} — your primary provider has reached its limit.{' '}
      <button type="button" onClick={onDismiss} className="button is-ghost is-small has-text-weight-medium" style={{ textDecoration: 'underline' }}>Got it</button>
    </div>
  );
}

export default function PostCard({ post, onApproved, onDismissed, isFocused = false, connectedPlatforms, hasUnsplashKey }: Props) {
  const isFailed = post.status === 'failed';
  const [expanded, setExpanded] = useState(isFailed);
  const [activeTab, setActiveTab] = useState<Platform>(isPlatform(post.platforms[0]) ? post.platforms[0] : 'x');
  const [localSchedule, setLocalSchedule] = useState<string | null>(post.schedule ?? null);
  const platforms = platformsOnPost(post);
  const { approving, approveError, approveBlockedInfo, approveSuccessPlatforms, onSuccessDismissed, fallbackNotice, dismissFallbackNotice, retrying, retryError, dismissError, approve, dismiss, retry } = usePostCardActions(post, onApproved, onDismissed);
  const handleKeyDown = usePostCardKeyboard(isFocused, isFailed, platforms, approve, dismiss, retry, setActiveTab, setExpanded);

  return (
    <article role="article" data-post-card onKeyDown={handleKeyDown} tabIndex={0}
      className={'box' + (isFocused ? ' has-background-info-light' : '')} style={{ padding: '1rem' }}>
      <div className="is-flex is-align-items-flex-start" style={{ gap: '0.75rem' }}>
        <div style={{ minWidth: 0, flex: 1 }}>
          <div className="tags mb-1" style={{ gap: '0.5rem' }}>
            <span className="tag is-light">{post.repo_name}</span>
            {isFailed && <span className="tag is-danger is-light">Failed</span>}
          </div>
          <p className="has-text-weight-medium is-size-7" style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{triggerText(post)}</p>
          <PostCardMeta post={post} localSchedule={localSchedule} connectedPlatforms={connectedPlatforms} />
        </div>
        <div className="is-flex is-align-items-center" style={{ gap: '0.5rem', flexShrink: 0 }}>
          <button className="button is-info is-small" onClick={() => setExpanded((v) => !v)} aria-label="Preview" aria-expanded={expanded}>
            <FontAwesomeIcon icon={faChevronDown} style={{ transform: expanded ? 'rotate(180deg)' : undefined }} />
            <span>Preview</span>
          </button>
        </div>
      </div>
      {isFailed && <FailedErrorBanner error={post.error} platformResults={post.platform_results} />}
      {approveSuccessPlatforms !== null && (
        <SendSuccessModal platforms={approveSuccessPlatforms} onClose={onSuccessDismissed} />
      )}
      {fallbackNotice && <FallbackNotice provider={fallbackNotice} onDismiss={dismissFallbackNotice} />}
      {expanded && platforms.length > 0 && (
        <PostCardBody post={post} platforms={platforms} activeTab={activeTab} isFailed={isFailed} approving={approving} approveError={approveError} approveBlockedInfo={approveBlockedInfo} retrying={retrying} retryError={retryError} dismissError={dismissError} schedule={localSchedule} hasUnsplashKey={hasUnsplashKey} onApprove={approve} onDelete={dismiss} onTabChange={setActiveTab} onScheduleChange={setLocalSchedule} />
      )}
    </article>
  );
}
