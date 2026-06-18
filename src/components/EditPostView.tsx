// SPDX-License-Identifier: BUSL-1.1

import { useDraftPostsContext } from '../context/DraftPostsProvider';
import { useEditGuard } from '../context/EditGuardContext';
import { CHAR_LIMITS } from './PreviewModal';
import { PLATFORM_CFG } from '../constants/platformConfig';
import { countCharsX, countCharsBluesky, countCharsMastodon, countLinkedInChars } from './charCount';
import type { DraftPost, PublishedPost, Project, ViewSelection } from '../types';
import EditPostDraftColumn from './EditPostDraftColumn';
import EditPostPreviewColumn from './EditPostPreviewColumn';
import {
  useTextState, usePostImage, useSavePost, useApproveHandlers,
  useDeletePost, useDiscardGuard, useEditKeyboard, usePlatformTabs, useTabSwitch,
} from '../hooks/usePostEditor';

// ── Types ─────────────────────────────────────────────────────────────────────

export interface EditPostViewProps {
  post: DraftPost | PublishedPost;
  project: Project;
  isHistory: boolean;
  timezone: string;
  onBack: () => void;
  onApproved: () => void;
  onNavigate: (_sel: ViewSelection) => void;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function countChars(platform: string, text: string): number {
  if (platform === 'x') return countCharsX(text);
  if (platform === 'bluesky') return countCharsBluesky(text);
  if (platform === 'mastodon') return countCharsMastodon(text);
  if (platform === 'linkedin') return countLinkedInChars(text);
  return [...text].length;
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

// ── Main component ────────────────────────────────────────────────────────────

export default function EditPostView({
  post, project, isHistory, onBack, onApproved, onNavigate,
}: EditPostViewProps) {
  const { setDirty, pendingNavSel, onNavCancelled } = useEditGuard();
  const { drafts, refresh } = useDraftPostsContext();
  const { siblings, platformList, selectedPlatform, setSelectedPlatform, currentPost } = usePlatformTabs(post, drafts);
  const { text, setText, originalTextRef, isDirty } = useTextState(post, setDirty);

  const limit = CHAR_LIMITS[selectedPlatform] ?? 0;
  const count = countChars(selectedPlatform, text);
  const isOverLimit = limit > 0 && count > limit;

  const handleTabSwitch = useTabSwitch(
    isDirty, post.repo_path, post.post_folder, selectedPlatform, text,
    siblings, originalTextRef, setSelectedPlatform, setText,
  );

  const save = useSavePost(currentPost, text, originalTextRef, refresh);
  const approve = useApproveHandlers(
    currentPost, siblings, selectedPlatform, refresh, onApproved,
    setSelectedPlatform, setText, originalTextRef);
  const del = useDeletePost(currentPost, selectedPlatform, onBack);
  const guard = useDiscardGuard(isDirty, onBack, onNavigate, pendingNavSel, onNavCancelled);
  const { imageState, handleSetImage, handleUnsplashSelect, handleRemoveImage } = usePostImage(
    post, text, isHistory, refresh, save.doSave,
  );
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
          charCount={count} charLimit={limit} isFailed={currentPost.status === 'failed'}
          approveLoading={approve.approveLoading} approveError={approve.approveError} doApprove={approve.doApprove}
        />
      </div>
      {del.deleteConfirm && <DeleteModal platform={selectedPlatform} onConfirm={del.confirmDelete}
        onCancel={del.cancelDelete} loading={del.deleteLoading} error={del.deleteError} />}
      {guard.pendingDiscard && <DiscardModal onDiscard={guard.handleDiscardConfirm} onCancel={guard.handleDiscardCancel} />}
    </div>
  );
}
