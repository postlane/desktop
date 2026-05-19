// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback, useEffect, useRef, type MutableRefObject } from 'react';
import { invoke } from '../ipc/invoke';
import { useDraftPostsContext } from '../context/DraftPostsProvider';
import { CHAR_LIMITS, CharCount } from './PreviewModal';
import { PLATFORM_CFG } from '../constants/platformConfig';
import PreviewModal from './PreviewModal';
import { countCharsX, countCharsBluesky, countCharsMastodon, countLinkedInChars } from './charCount';
import type { DraftPost, PublishedPost, Project, ViewSelection, ImageState } from '../types';

// ── Types ─────────────────────────────────────────────────────────────────────

export interface EditPostViewProps {
  post: DraftPost | PublishedPost;
  project: Project;
  isHistory: boolean;
  timezone: string;
  onBack: () => void;
  onApproved: () => void;
  onToast: (_msg: string, _durationMs: number) => void;
  onNavigate: (_sel: ViewSelection) => void;
  onDirtyChange?: (_dirty: boolean) => void;
  pendingNavSel?: ViewSelection | null;
  onNavCancelled?: () => void;
}

type PendingDiscard = { type: 'back' } | { type: 'nav'; dest: ViewSelection };

// ── Helpers ───────────────────────────────────────────────────────────────────

function countChars(platform: string, text: string): number {
  if (platform === 'x') return countCharsX(text);
  if (platform === 'bluesky') return countCharsBluesky(text);
  if (platform === 'mastodon') return countCharsMastodon(text);
  if (platform === 'linkedin') return countLinkedInChars(text);
  return [...text].length;
}

function isDraft(post: DraftPost | PublishedPost): post is DraftPost {
  return post.status === 'ready' || post.status === 'failed';
}

function approveTooltip(
  isDirty: boolean, billingInactive: boolean, isOverLimit: boolean,
  label: string, count: number, limit: number,
): string | null {
  if (isDirty) return 'Save your changes before approving.';
  if (billingInactive) return 'Billing inactive — update payment at postlane.dev/billing';
  if (isOverLimit) return `Post exceeds the ${label} character limit (${count}/${limit})`;
  return null;
}

// ── Sub-components ────────────────────────────────────────────────────────────

function DiscardModal({ onDiscard, onCancel }: { onDiscard: () => void; onCancel: () => void }) {
  return (
    <div role="dialog" aria-modal="true" className="modal is-active">
      <div className="modal-background" />
      <div className="modal-card">
        <section className="modal-card-body">
          <p className="is-size-6 has-text-weight-medium">Discard unsaved changes?</p>
          <p className="is-size-7 has-text-grey mt-2">Your edits will be lost.</p>
        </section>
        <footer className="modal-card-foot" style={{ gap: '0.5rem' }}>
          <button className="button is-danger" onClick={onDiscard}>Discard</button>
          <button className="button" onClick={onCancel}>Cancel</button>
        </footer>
      </div>
    </div>
  );
}

function DeleteModal({ platform, onConfirm, onCancel, loading, error }: {
  platform: string; onConfirm: () => void; onCancel: () => void;
  loading: boolean; error: string | null;
}) {
  const label = PLATFORM_CFG[platform]?.label ?? platform;
  return (
    <div role="dialog" aria-modal="true" className="modal is-active">
      <div className="modal-background" onClick={onCancel} />
      <div className="modal-card">
        <section className="modal-card-body">
          <p>Delete this {label} post? Other platforms in this draft will not be affected.</p>
          {error && <p className="has-text-danger mt-2">{error}</p>}
        </section>
        <footer className="modal-card-foot" style={{ gap: '0.5rem' }}>
          <button className="button is-danger" onClick={onConfirm} disabled={loading} data-testid="confirm-delete">Delete</button>
          <button className="button" onClick={onCancel}>Cancel</button>
        </footer>
      </div>
    </div>
  );
}

function ActionBar({ platform, isDirty, isOverLimit, isHistory, project, isFailed, doSave,
  saveLoading, doApprove, approveLoading, onDelete, onPreview, charCount, charLimit }: {
  platform: string; isDirty: boolean; isOverLimit: boolean; isHistory: boolean;
  project: Project; isFailed: boolean; doSave: () => void; saveLoading: boolean;
  doApprove: () => void; approveLoading: boolean; onDelete: () => void;
  onPreview: () => void; charCount: number; charLimit: number;
}) {
  const label = PLATFORM_CFG[platform]?.label ?? platform;
  const tip = approveTooltip(isDirty, !project.billing_active, isOverLimit, label, charCount, charLimit);
  if (isHistory) {
    return (
      <div className="is-flex is-align-items-center" style={{ gap: '0.5rem', padding: '0.75rem' }}>
        <button className="button is-small" disabled title="Re-run /draft-post to create a new draft.">Repost</button>
        <span className="is-size-7 has-text-grey">Post analytics — coming in v2.</span>
        <button className="button is-small has-background-link has-text-white" onClick={onPreview}>Preview</button>
      </div>
    );
  }
  return (
    <div className="is-flex is-align-items-center" style={{ gap: '0.5rem', padding: '0.75rem' }}>
      <button className="button is-small has-background-link has-text-white" onClick={onPreview}>Preview</button>
      <button className="button is-small has-background-danger has-text-white" onClick={onDelete} aria-label="Delete">Delete</button>
      <button className={`button is-small ${isDirty ? 'has-background-warning has-text-white' : 'has-background-white-ter has-text-grey'}`}
        onClick={doSave} disabled={saveLoading}>Save</button>
      <button className="button is-small has-background-success has-text-white" disabled={!!tip || approveLoading}
        title={tip ?? undefined} onClick={doApprove} aria-label={isFailed ? 'Retry' : 'Approve'}>
        {isFailed ? 'Retry' : 'Approve'}
      </button>
    </div>
  );
}

function ImageSection({ imageState, onCustomSet }: {
  imageState: ImageState; onCustomSet: (_url: string) => Promise<void>;
}) {
  const [customUrl, setCustomUrl] = useState('');
  const [customError, setCustomError] = useState<string | null>(null);
  const [customLoading, setCustomLoading] = useState(false);
  async function handleSet() {
    setCustomError(null);
    if (!customUrl.startsWith('https://')) { setCustomError('URL must start with https://'); return; }
    setCustomLoading(true);
    try {
      await invoke('validate_url_safe', { url: customUrl });
      await onCustomSet(customUrl);
    } catch (e: unknown) {
      setCustomError(String(e));
    } finally {
      setCustomLoading(false);
    }
  }
  return (
    <div className="px-4 py-2">
      {imageState.status === 'loaded' && (
        <img data-testid="og-image" src={imageState.url} alt="Post image"
          style={{ maxWidth: '100%', maxHeight: 140, objectFit: 'cover', borderRadius: 4, marginBottom: '0.5rem' }} />
      )}
      {imageState.status === 'loading' && <span className="is-size-7 has-text-grey">Loading image…</span>}
      <div className="is-flex is-align-items-center mt-2" style={{ gap: '0.5rem' }}>
        <input type="url" aria-label="Custom image URL" value={customUrl}
          onChange={(e) => setCustomUrl(e.target.value)} placeholder="https://…"
          className="input is-small" style={{ flex: 1 }} />
        <button className="button is-small is-light" onClick={handleSet} disabled={customLoading}
          data-testid="set-custom-image">Set image</button>
      </div>
      {customError && <p role="alert" className="is-size-7 has-text-danger mt-1">{customError}</p>}
    </div>
  );
}

function PostBody({ isHistory, text, setText, post, saveError, approveError }: {
  isHistory: boolean; text: string; setText: (_v: string) => void;
  post: DraftPost | PublishedPost; saveError: string | null; approveError: string | null;
}) {
  return (
    <div className="px-4 py-3" style={{ flex: 1 }}>
      {isHistory
        ? <div data-testid="post-text" className="is-size-7">{text}</div>
        : <textarea className="textarea is-size-7" aria-label="Post content" rows={10} value={text} onChange={(e) => setText(e.target.value)} />}
      {isDraft(post) && post.status === 'failed' && post.error && <p role="alert" className="is-size-7 has-text-danger mt-2">{post.error}</p>}
      {saveError && <p role="alert" className="is-size-7 has-text-danger mt-1">{saveError}</p>}
      {approveError && <p role="alert" className="is-size-7 has-text-danger mt-1">{approveError}</p>}
    </div>
  );
}

// ── Hooks ─────────────────────────────────────────────────────────────────────

function useOgDetection(text: string, disabled: boolean): ImageState {
  const [state, setState] = useState<ImageState>({ status: 'none' });
  useEffect(() => {
    if (disabled) { setState({ status: 'none' }); return; }
    const match = text.match(/https:\/\/[^\s]+/);
    if (!match) { setState({ status: 'none' }); return; }
    const url = match[0];
    setState({ status: 'loading' });
    const timer = setTimeout(() => {
      invoke<string | null>('fetch_og_image', { url })
        .then((ogUrl) => setState(ogUrl ? { status: 'loaded', url: ogUrl } : { status: 'none' }))
        .catch(() => setState({ status: 'none' }));
    }, 500);
    return () => clearTimeout(timer);
  }, [text, disabled]);
  return state;
}

function usePostImage(post: DraftPost | PublishedPost, text: string, isHistory: boolean) {
  const initialUrl = 'image_url' in post ? (post.image_url ?? null) : null;
  const [customImageUrl, setCustomImageUrl] = useState<string | null>(initialUrl);
  const ogState = useOgDetection(text, !!customImageUrl || isHistory);
  const imageState: ImageState = customImageUrl ? { status: 'loaded', url: customImageUrl } : ogState;

  const handleSetImage = useCallback(async (url: string) => {
    await invoke('update_post_image', { repoPath: post.repo_path, postFolder: post.post_folder, imageUrl: url });
    setCustomImageUrl(url);
  }, [post.repo_path, post.post_folder]);

  return { imageState, handleSetImage };
}

function useSavePost(
  post: DraftPost | PublishedPost,
  text: string,
  originalTextRef: MutableRefObject<string>,
) {
  const [saveLoading, setSaveLoading] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const doSave = useCallback(async () => {
    setSaveLoading(true);
    setSaveError(null);
    try {
      await invoke('save_post_draft', {
        repoPath: post.repo_path, postFolder: post.post_folder,
        platform: post.platform ?? '', text,
      });
      originalTextRef.current = text;
    } catch (e: unknown) {
      setSaveError(String(e));
    } finally {
      setSaveLoading(false);
    }
  }, [post, text, originalTextRef]);
  return { doSave, saveLoading, saveError };
}

function useApprovePost(
  post: DraftPost | PublishedPost,
  _project: Project,
  refresh: () => void,
  onApproved: () => void,
  onToast: (_msg: string, _ms: number) => void,
) {
  const [approveLoading, setApproveLoading] = useState(false);
  const [approveError, setApproveError] = useState<string | null>(null);
  const doApprove = useCallback(async () => {
    setApproveLoading(true);
    setApproveError(null);
    try {
      await invoke('approve_post', {
        repoPath: post.repo_path, postFolder: post.post_folder, platform: post.platform ?? '',
      });
      refresh();
      onApproved();
      onToast('Post approved.', 3000);
    } catch (e: unknown) {
      setApproveError(String(e));
    } finally {
      setApproveLoading(false);
    }
  }, [post, refresh, onApproved, onToast]);
  return { doApprove, approveLoading, approveError };
}

function useDeletePost(post: DraftPost | PublishedPost, platform: string, onBack: () => void) {
  const { refresh } = useDraftPostsContext();
  const [deleteConfirm, setDeleteConfirm] = useState(false);
  const [deleteLoading, setDeleteLoading] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  async function confirmDelete() {
    setDeleteLoading(true);
    setDeleteError(null);
    try {
      await invoke('delete_post', { repoPath: post.repo_path, postFolder: post.post_folder, platform });
      refresh();
      onBack();
    } catch (e: unknown) {
      setDeleteError(String(e));
      setDeleteLoading(false);
    }
  }
  return {
    deleteConfirm, requestDelete: () => setDeleteConfirm(true), cancelDelete: () => setDeleteConfirm(false),
    confirmDelete, deleteLoading, deleteError,
  };
}

function useDiscardGuard(
  isDirty: boolean, onBack: () => void, onNavigate: (_sel: ViewSelection) => void,
  pendingNavSel: ViewSelection | null | undefined, onNavCancelled: (() => void) | undefined,
) {
  const [pendingDiscard, setPendingDiscard] = useState<PendingDiscard | null>(null);
  const isDirtyRef = useRef(isDirty);
  isDirtyRef.current = isDirty;
  useEffect(() => {
    if (pendingNavSel == null) return;
    if (isDirtyRef.current) {
      setPendingDiscard({ type: 'nav', dest: pendingNavSel });
    } else {
      onNavigate(pendingNavSel);
    }
  }, [pendingNavSel, onNavigate]);
  function handleBack() {
    if (isDirtyRef.current) { setPendingDiscard({ type: 'back' }); return; }
    onBack();
  }
  function handleDiscardConfirm() {
    const dest = pendingDiscard;
    setPendingDiscard(null);
    if (dest?.type === 'back') { onBack(); return; }
    if (dest?.type === 'nav') onNavigate(dest.dest);
  }
  function handleDiscardCancel() {
    setPendingDiscard(null);
    if (pendingNavSel != null) onNavCancelled?.();
  }
  return { pendingDiscard, handleBack, handleDiscardConfirm, handleDiscardCancel };
}

function useEditKeyboard(
  isDirty: boolean, isHistory: boolean, isOverLimit: boolean,
  doSave: () => void, doApprove: () => void,
) {
  useEffect(() => {
    function handler(e: KeyboardEvent) {
      if (!(e.metaKey || e.ctrlKey) || e.key !== 'Enter') return;
      e.preventDefault();
      if (isDirty) { doSave(); return; }
      if (!isHistory && !isOverLimit) doApprove();
    }
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [isDirty, isHistory, isOverLimit, doSave, doApprove]);
}

function EditPostDialogs({ del, guard, previewOpen, onClosePreview, platform, text, imageState }: {
  del: ReturnType<typeof useDeletePost>;
  guard: ReturnType<typeof useDiscardGuard>;
  previewOpen: boolean; onClosePreview: () => void;
  platform: string; text: string; imageState: ImageState;
}) {
  return (
    <>
      {del.deleteConfirm && <DeleteModal platform={platform} onConfirm={del.confirmDelete}
        onCancel={del.cancelDelete} loading={del.deleteLoading} error={del.deleteError} />}
      {guard.pendingDiscard && <DiscardModal onDiscard={guard.handleDiscardConfirm} onCancel={guard.handleDiscardCancel} />}
      {previewOpen && <PreviewModal platform={platform} text={text} imageState={imageState} onClose={onClosePreview} />}
    </>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

export default function EditPostView({
  post, project, isHistory, onBack, onApproved, onToast, onNavigate, onDirtyChange, pendingNavSel, onNavCancelled,
}: EditPostViewProps) {
  const { refresh } = useDraftPostsContext();
  const initialText = post.text ?? '';
  const [text, setText] = useState(initialText);
  const originalTextRef = useRef(initialText);
  const isDirty = text !== originalTextRef.current;
  useEffect(() => { onDirtyChange?.(isDirty); }, [isDirty, onDirtyChange]);
  useEffect(() => () => { onDirtyChange?.(false); }, [onDirtyChange]);
  const platform = post.platform ?? '';
  const platformCfg = PLATFORM_CFG[platform];
  const platformColor = platformCfg?.color ?? 'hsl(0,0%,50%)';
  const platformLabel = platformCfg?.label ?? platform;
  const limit = CHAR_LIMITS[platform] ?? 0;
  const count = countChars(platform, text);
  const isOverLimit = limit > 0 && count > limit;
  const isFailed = post.status === 'failed';
  const save = useSavePost(post, text, originalTextRef);
  const approve = useApprovePost(post, project, refresh, onApproved, onToast);
  const del = useDeletePost(post, platform, onBack);
  const guard = useDiscardGuard(isDirty, onBack, onNavigate, pendingNavSel, onNavCancelled);
  const [previewOpen, setPreviewOpen] = useState(false);
  const { imageState, handleSetImage } = usePostImage(post, text, isHistory);
  useEditKeyboard(isDirty, isHistory, isOverLimit, save.doSave, approve.doApprove);
  return (
    <div className="is-flex" style={{ flexDirection: 'column', height: '100%' }}>
      <div className="is-flex is-align-items-center px-4 py-3" style={{ gap: '0.75rem', borderBottom: '1px solid var(--bulma-border-weak)' }}>
        <button className="button is-small is-ghost" onClick={guard.handleBack} aria-label="Back">Back</button>
        <span className="tag is-rounded is-small" data-testid="platform-badge"
          style={{ background: platformColor, color: '#fff', flexShrink: 0 }}>
          {platformLabel}
        </span>
        <span className="is-size-7 has-text-grey">{post.post_folder}</span>
        <CharCount platform={platform} text={text} />
      </div>
      <PostBody isHistory={isHistory} text={text} setText={setText} post={post}
        saveError={save.saveError} approveError={approve.approveError} />
      {!isHistory && <ImageSection imageState={imageState} onCustomSet={handleSetImage} />}
      <ActionBar platform={platform} isDirty={isDirty} isOverLimit={isOverLimit} isHistory={isHistory}
        project={project} isFailed={isFailed} doSave={save.doSave} saveLoading={save.saveLoading}
        doApprove={approve.doApprove} approveLoading={approve.approveLoading}
        onDelete={del.requestDelete} onPreview={() => setPreviewOpen(true)}
        charCount={count} charLimit={limit} />
      <EditPostDialogs del={del} guard={guard} previewOpen={previewOpen} onClosePreview={() => setPreviewOpen(false)}
        platform={platform} text={text} imageState={imageState} />
    </div>
  );
}
