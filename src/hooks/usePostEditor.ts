// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback, useEffect, useRef, useMemo, type MutableRefObject } from 'react';
import { invoke } from '../ipc/invoke';
import { useDraftPostsContext } from '../context/DraftPostsProvider';
import { useAsyncCommand } from './useAsyncCommand';
import type { DraftPost, PublishedPost, ViewSelection, ImageState, ImageAttribution } from '../types';

// ── Types ─────────────────────────────────────────────────────────────────────

type PendingDiscard = { type: 'back' } | { type: 'nav'; dest: ViewSelection };

// ── Helpers ───────────────────────────────────────────────────────────────────

function isDraftPost(post: DraftPost | PublishedPost): post is DraftPost {
  return post.status === 'ready' || post.status === 'failed';
}

// ── Hooks ─────────────────────────────────────────────────────────────────────

export function useTextState(
  post: DraftPost | PublishedPost,
  onDirtyChange: (_d: boolean) => void,
) {
  const initialText = post.text ?? '';
  const [text, setText] = useState(initialText);
  const originalTextRef = useRef(initialText);
  const isDirty = text !== originalTextRef.current;
  useEffect(() => { onDirtyChange(isDirty); }, [isDirty, onDirtyChange]);
  useEffect(() => () => { onDirtyChange(false); }, [onDirtyChange]);
  return { text, setText, originalTextRef, isDirty };
}

export function useOgDetection(text: string, disabled: boolean): ImageState {
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

export function usePostImage(
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
  const handleUnsplashSelect = useCallback(async (
    url: string, downloadLocation: string, attr: ImageAttribution,
  ) => {
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

export function useSavePost(
  post: DraftPost | PublishedPost,
  text: string,
  originalTextRef: MutableRefObject<string>,
  refresh: () => void,
) {
  const { loading: saveLoading, error: saveError, run: runSave } = useAsyncCommand();
  const doSave = useCallback(async () => {
    const result = await runSave(() => invoke('save_post_draft', {
      repoPath: post.repo_path, postFolder: post.post_folder,
      platform: post.platform ?? '', text,
    }));
    if (result !== null) {
      originalTextRef.current = text;
      refresh();
    }
  }, [post, text, originalTextRef, refresh, runSave]);
  return { doSave, saveLoading, saveError };
}

export function useApproveHandlers(
  post: DraftPost | PublishedPost,
  siblings: DraftPost[],
  selectedPlatform: string,
  refresh: () => void,
  onApproved: () => void,
  setSelectedPlatform: (_p: string) => void,
  setText: (_t: string) => void,
  originalTextRef: MutableRefObject<string>,
) {
  const { loading: approveLoading, error: approveError, run: runApprove } = useAsyncCommand();
  const doApprove = useCallback(async () => {
    const result = await runApprove(() => invoke('approve_post', {
      repoPath: post.repo_path, postFolder: post.post_folder, platform: post.platform ?? '',
    }));
    if (result !== null) {
      const remaining = siblings.filter(s => s.platform !== selectedPlatform);
      refresh();
      if (remaining.length > 0) {
        const next = remaining[0];
        setSelectedPlatform(next.platform ?? '');
        originalTextRef.current = next.text ?? '';
        setText(next.text ?? '');
      } else { onApproved(); }
    }
  }, [post, siblings, selectedPlatform, refresh, onApproved, setSelectedPlatform, setText, originalTextRef, runApprove]);
  return { doApprove, approveLoading, approveError };
}

export function useDeletePost(
  post: DraftPost | PublishedPost, platform: string, onBack: () => void,
) {
  const { refresh } = useDraftPostsContext();
  const [deleteConfirm, setDeleteConfirm] = useState(false);
  const { loading: deleteLoading, error: deleteError, run: runDelete } = useAsyncCommand();
  async function confirmDelete() {
    const result = await runDelete(() =>
      invoke('delete_post', { repoPath: post.repo_path, postFolder: post.post_folder, platform }),
    );
    if (result !== null) {
      refresh();
      onBack();
    }
  }
  return {
    deleteConfirm, requestDelete: () => setDeleteConfirm(true), cancelDelete: () => setDeleteConfirm(false),
    confirmDelete, deleteLoading, deleteError,
  };
}

export function useDiscardGuard(
  isDirty: boolean, onBack: () => void, onNavigate: (_sel: ViewSelection) => void,
  pendingNavSel: ViewSelection | null, onNavCancelled: () => void,
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
    if (pendingNavSel != null) onNavCancelled();
  }
  return { pendingDiscard, handleBack, handleDiscardConfirm, handleDiscardCancel };
}

export function useEditKeyboard(
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

export function usePlatformTabs(
  post: DraftPost | PublishedPost, drafts: (DraftPost | PublishedPost)[],
) {
  const siblings = useMemo(
    () => isDraftPost(post)
      ? (drafts.filter(
          d => d.post_folder === post.post_folder && d.repo_id === post.repo_id && isDraftPost(d),
        ) as DraftPost[])
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
