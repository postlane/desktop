// SPDX-License-Identifier: BUSL-1.1

import { PLATFORM_CFG } from '../constants/platformConfig';
import { ImageDisplay } from './EditPostImageSection';
import { CharCount } from './PreviewModal';
import type { DraftPost, PublishedPost, Project, ImageState } from '../types';

function approveTooltip(
  isDirty: boolean, billingInactive: boolean, isOverLimit: boolean,
  label: string, count: number, limit: number,
): string | null {
  if (isDirty) return 'Save your changes before approving.';
  if (billingInactive) return 'Billing inactive — update payment at postlane.dev/billing';
  if (isOverLimit) return `Post exceeds the ${label} character limit (${count}/${limit})`;
  return null;
}

export interface PreviewColumnProps {
  post: DraftPost | PublishedPost;
  text: string;
  imageState: ImageState;
  isHistory: boolean;
  project: Project;
  isDirty: boolean;
  isOverLimit: boolean;
  charCount: number;
  charLimit: number;
  isFailed: boolean;
  approveLoading: boolean;
  approveError: string | null;
  doApprove: () => void;
}

export default function EditPostPreviewColumn({
  post, text, imageState, isHistory, project, isDirty, isOverLimit,
  charCount, charLimit, isFailed, approveLoading, approveError, doApprove,
}: PreviewColumnProps) {
  const platform = post.platform ?? '';
  const label = PLATFORM_CFG[platform]?.label ?? platform;
  const tip = approveTooltip(isDirty, !project.billing_active, isOverLimit, label, charCount, charLimit);
  const approveLabel = isFailed ? 'Retry' : 'Approve';
  const tipTitle = tip ?? undefined;
  return (
    <div data-testid="preview-column" style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
      <div className="is-flex is-align-items-center px-4" style={{ borderBottom: '1px solid var(--bulma-border-weak)', flexShrink: 0, minHeight: '3rem' }}>
        <span className="is-size-7 has-text-weight-semibold">Preview</span>
      </div>
      <div style={{ flex: 1, overflowY: 'auto', padding: '0.75rem 1rem' }}>
        {imageState.status === 'loading' && (
          <span className="is-size-7 has-text-grey">Loading image…</span>
        )}
        <ImageDisplay imageState={imageState} />
        <p data-testid="preview-text" className="is-size-7" style={{ whiteSpace: 'pre-wrap' }}>{text}</p>
        <div className="is-flex is-justify-content-flex-end mt-1">
          <CharCount platform={platform} text={text} />
        </div>
      </div>
      <div style={{ padding: '0.75rem 1rem', borderTop: '1px solid var(--bulma-border-weak)', flexShrink: 0 }}>
        {approveError && <p role="alert" className="is-size-7 has-text-danger mb-2">{approveError}</p>}
        {isHistory ? (
          <div>
            <span className="is-size-7 has-text-grey">Post analytics — coming in v2.</span>
          </div>
        ) : (
          <button
            className="button is-small is-fullwidth has-background-success has-text-white"
            style={{ border: 'none' }} disabled={!!tip || approveLoading} title={tipTitle}
            onClick={doApprove} aria-label={approveLabel}>
            {approveLabel}
          </button>
        )}
      </div>
    </div>
  );
}
