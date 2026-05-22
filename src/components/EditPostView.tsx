// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback, useEffect, useRef, useMemo, type MutableRefObject } from 'react';
import { invoke } from '../ipc/invoke';
import { useDraftPostsContext } from '../context/DraftPostsProvider';
import { CHAR_LIMITS } from './PreviewModal';
import { PLATFORM_CFG } from '../constants/platformConfig';
import { countCharsX, countCharsBluesky, countCharsMastodon, countLinkedInChars } from './charCount';
import type { DraftPost, PublishedPost, Project, ViewSelection, ImageState, ImageAttribution } from '../types';
import EditPostDraftColumn from './EditPostDraftColumn';
import EditPostPreviewColumn from './EditPostPreviewColumn';

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

function isDraftPost(post: DraftPost | PublishedPost): post is DraftPost {
  return post.status === 'ready' || post.status === 'failed';
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

function usePostImage(
  post: DraftPost | PublishedPost, text: string, isHistory: boolean,
  refresh: () => void, doSave: () => Promise<void>,
) {
  const initialUrl = 'image_url' in post ? (post.image_url ?? null) : null;
  const initialAttribution = 'image_attribution' in post ? (post.image_attribution ?? null) : null;
  const [customImageUrl, setCustomImageUrl] = useState<string | null>(initialUrl);
  const [customAttribution, setCustomAttribution] = useState<ImageAttribution | null>(initialAttribution);
  const ogState = useOgDetection(text, !!customImageUrl || isHistory);
  const imageState: ImageState = customImageUrl
    ? { status: 'loaded', url: customImageUrl, attribution: customAttribution }
    : ogState;
  const handleSetImage = useCallback(async (url: string) => {
    await invoke('update_post_image', { repoPath: post.repo_path, postFolder: post.post_folder, imageUrl: url });
    setCustomImageUrl(url);
    setCustomAttribution(null);
    refresh();
  }, [post.repo_path, post.post_folder, refresh]);
  const handleUnsplashSelect = useCallback(async (url: string, downloadLocation: string, attr: ImageAttribution) => {
    await invoke('update_post_image_unsplash', {
      repoPath: post.repo_path, postFolder: post.post_folder,
      imageUrl: url, downloadLocation,
      photographerName: attr.photographer_name, photographerUrl: attr.photographer_url,
    });
    setCustomImageUrl(url);
    setCustomAttribution(attr);
    invoke('trigger_unsplash_download', { downloadLocation }).catch(console.error);
    await doSave();
    refresh();
  }, [post.repo_path, post.post_folder, doSave, refresh]);
  const handleRemoveImage = useCallback(async () => {
    await invoke('update_post_image', { repoPath: post.repo_path, postFolder: post.post_folder, imageUrl: null });
    setCustomImageUrl(null);
    setCustomAttribution(null);
    refresh();
  }, [post.repo_path, post.post_folder, refresh]);
  return { imageState, handleSetImage, handleUnsplashSelect, handleRemoveImage };
}

function useSavePost(
  post: DraftPost | PublishedPost,
  text: string,
  originalTextRef: MutableRefObject<string>,
  refresh: () => void,
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
      refresh();
    } catch (e: unknown) {
      setSaveError(String(e));
    } finally {
      setSaveLoading(false);
    }
  }, [post, text, originalTextRef, refresh]);
  return { doSave, saveLoading, saveError };
}

function useApproveHandlers(
  post: DraftPost | PublishedPost,
  siblings: DraftPost[],
  selectedPlatform: string,
  refresh: () => void,
  onApproved: () => void,
  onToast: (_msg: string, _ms: number) => void,
  setSelectedPlatform: (_p: string) => void,
  setText: (_t: string) => void,
  originalTextRef: MutableRefObject<string>,
) {
  const [approveLoading, setApproveLoading] = useState(false);
  const [approveError, setApproveError] = useState<string | null>(null);
  const doApprove = useCallback(async () => {
    setApproveLoading(true); setApproveError(null);
    try {
      await invoke('approve_post', {
        repoPath: post.repo_path, postFolder: post.post_folder, platform: post.platform ?? '',
      });
      const remaining = siblings.filter(s => s.platform !== selectedPlatform);
      refresh();
      if (remaining.length > 0) {
        const next = remaining[0];
        setSelectedPlatform(next.platform ?? '');
        originalTextRef.current = next.text ?? '';
        setText(next.text ?? '');
      } else { onApproved(); }
      onToast('Post approved.', 3000);
    } catch (e: unknown) { setApproveError(String(e)); }
    finally { setApproveLoading(false); }
  }, [post, siblings, selectedPlatform, refresh, onApproved, onToast, setSelectedPlatform, setText, originalTextRef]);
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

function usePlatformTabs(post: DraftPost | PublishedPost, drafts: (DraftPost | PublishedPost)[]) {
  const siblings = useMemo(
    () => isDraftPost(post)
      ? (drafts.filter(d => d.post_folder === post.post_folder && d.repo_id === post.repo_id && isDraftPost(d)) as DraftPost[])
      : [],
    [drafts, post],
  );
  const platformList = useMemo(
    () => siblings.length > 0
      ? siblings.map(s => s.platform ?? '').filter(p => p !== '')
      : (post.platforms ?? []),
    [siblings, post],
  );
  const [selectedPlatform, setSelectedPlatform] = useState(post.platform ?? '');
  const currentPost: DraftPost | PublishedPost = useMemo(
    () => siblings.find(d => d.platform === selectedPlatform) ?? post,
    [siblings, selectedPlatform, post],
  );
  return { siblings, platformList, selectedPlatform, setSelectedPlatform, currentPost };
}

function useTextState(post: DraftPost | PublishedPost, onDirtyChange: ((_d: boolean) => void) | undefined) {
  const initialText = post.text ?? '';
  const [text, setText] = useState(initialText);
  const originalTextRef = useRef(initialText);
  const isDirty = text !== originalTextRef.current;
  useEffect(() => { onDirtyChange?.(isDirty); }, [isDirty, onDirtyChange]);
  useEffect(() => () => { onDirtyChange?.(false); }, [onDirtyChange]);
  return { text, setText, originalTextRef, isDirty };
}

// ── Main component ────────────────────────────────────────────────────────────

export default function EditPostView({
  post, project, isHistory, onBack, onApproved, onToast, onNavigate, onDirtyChange, pendingNavSel, onNavCancelled,
}: EditPostViewProps) {
  const { drafts, refresh } = useDraftPostsContext();
  const { siblings, platformList, selectedPlatform, setSelectedPlatform, currentPost } = usePlatformTabs(post, drafts);
  const { text, setText, originalTextRef, isDirty } = useTextState(post, onDirtyChange);

  const limit = CHAR_LIMITS[selectedPlatform] ?? 0;
  const count = countChars(selectedPlatform, text);
  const isOverLimit = limit > 0 && count > limit;
  const isFailed = currentPost.status === 'failed';

  const handleTabSwitch = useCallback(async (newPlatform: string) => {
    if (isDirty) {
      await invoke('save_post_draft', {
        repoPath: post.repo_path, postFolder: post.post_folder, platform: selectedPlatform, text,
      });
    }
    const sibling = siblings.find(d => d.platform === newPlatform);
    const newText = sibling?.text ?? '';
    originalTextRef.current = newText;
    setText(newText);
    setSelectedPlatform(newPlatform);
  }, [isDirty, post.repo_path, post.post_folder, selectedPlatform, text, siblings]);

  const save = useSavePost(currentPost, text, originalTextRef, refresh);
  const approve = useApproveHandlers(
    currentPost, siblings, selectedPlatform, refresh, onApproved, onToast,
    setSelectedPlatform, setText, originalTextRef);
  const del = useDeletePost(currentPost, selectedPlatform, onBack);
  const guard = useDiscardGuard(isDirty, onBack, onNavigate, pendingNavSel, onNavCancelled);
  const { imageState, handleSetImage, handleUnsplashSelect, handleRemoveImage } = usePostImage(post, text, isHistory, refresh, save.doSave);
  useEditKeyboard(isDirty, isHistory, isOverLimit, save.doSave, approve.doApprove);

  return (
    <div className="is-flex" style={{ flexDirection: 'column', height: '100%' }}>
      <div className="is-flex is-align-items-center px-4 py-3" style={{ gap: '0.75rem', borderBottom: '1px solid var(--bulma-border-weak)', flexShrink: 0 }}>
        <button className="button is-small is-ghost" onClick={guard.handleBack} aria-label="Back">Back</button>
        <span className="is-size-7 has-text-grey">{post.post_folder}</span>
      </div>
      <div className="is-flex" style={{ flex: 1, overflow: 'hidden' }}>
        <EditPostDraftColumn
          post={currentPost} platforms={platformList} selectedPlatform={selectedPlatform}
          text={text} isHistory={isHistory} imageState={imageState} isDirty={isDirty}
          saveLoading={save.saveLoading} saveError={save.saveError}
          onTextChange={setText} onTabSwitch={handleTabSwitch}
          onCustomSet={handleSetImage} onUnsplashSelect={handleUnsplashSelect} onRemove={handleRemoveImage}
          doSave={save.doSave} onDelete={del.requestDelete}
        />
        <EditPostPreviewColumn
          post={currentPost} text={text} imageState={imageState} isHistory={isHistory}
          project={project} isDirty={isDirty} isOverLimit={isOverLimit}
          charCount={count} charLimit={limit} isFailed={isFailed}
          approveLoading={approve.approveLoading} approveError={approve.approveError} doApprove={approve.doApprove}
        />
      </div>
      {del.deleteConfirm && <DeleteModal platform={selectedPlatform} onConfirm={del.confirmDelete}
        onCancel={del.cancelDelete} loading={del.deleteLoading} error={del.deleteError} />}
      {guard.pendingDiscard && <DiscardModal onDiscard={guard.handleDiscardConfirm} onCancel={guard.handleDiscardCancel} />}
    </div>
  );
}
