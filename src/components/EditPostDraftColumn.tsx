// SPDX-License-Identifier: BUSL-1.1

import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faXTwitter, faBluesky, faMastodon, faLinkedinIn } from '@fortawesome/free-brands-svg-icons';
import type { IconDefinition } from '@fortawesome/fontawesome-svg-core';
import { PLATFORM_CFG } from '../constants/platformConfig';
import { ImagePickers } from './EditPostImageSection';
import type { DraftPost, PublishedPost, ImageState, ImageAttribution } from '../types';

const PLATFORM_ICONS: Record<string, IconDefinition> = {
  x: faXTwitter,
  bluesky: faBluesky,
  mastodon: faMastodon,
  linkedin: faLinkedinIn,
};

function isDraft(post: DraftPost | PublishedPost): post is DraftPost {
  return post.status === 'ready' || post.status === 'failed';
}

function PlatformTabs({ platforms, selected, onSwitch }: {
  platforms: string[]; selected: string; onSwitch: (_p: string) => void;
}) {
  return (
    <div role="tablist" className="is-flex" style={{ gap: '0.25rem' }}>
      {platforms.map((p) => {
        const cfg = PLATFORM_CFG[p];
        const icon = PLATFORM_ICONS[p];
        const isSelected = p === selected;
        const label = cfg?.label ?? p;
        return (
          <button
            key={p}
            role="tab"
            aria-selected={isSelected}
            aria-label={label}
            title={label}
            onClick={() => onSwitch(p)}
            className={`button is-small ${isSelected ? '' : 'is-ghost has-text-grey'}`}
            style={{ width: '2rem', height: '2rem', padding: 0, borderRadius: '50%', flexShrink: 0,
              ...(isSelected ? { background: cfg?.color ?? 'hsl(0,0%,50%)', color: '#fff' } : {}) }}>
            {icon ? <FontAwesomeIcon icon={icon} /> : label}
          </button>
        );
      })}
    </div>
  );
}

export interface DraftColumnProps {
  post: DraftPost | PublishedPost;
  platforms: string[];
  selectedPlatform: string;
  text: string;
  isHistory: boolean;
  imageState: ImageState;
  isDirty: boolean;
  saveLoading: boolean;
  saveError: string | null;
  onTextChange: (_v: string) => void;
  onTabSwitch: (_p: string) => void;
  onCustomSet: (_url: string) => Promise<void>;
  onUnsplashSelect: (_url: string, _dl: string, _attr: ImageAttribution) => Promise<void>;
  onRemove: () => Promise<void>;
  doSave: () => void;
  onDelete: () => void;
}

export default function EditPostDraftColumn({
  post, platforms, selectedPlatform, text, isHistory, imageState,
  isDirty, saveLoading, saveError, onTextChange, onTabSwitch,
  onCustomSet, onUnsplashSelect, onRemove, doSave, onDelete,
}: DraftColumnProps) {
  const showTabs = !isHistory && platforms.length > 0;
  return (
    <div style={{ flex: 1, display: 'flex', flexDirection: 'column', borderRight: '1px solid var(--bulma-border-weak)', overflow: 'hidden' }}>
      <div className="is-flex is-align-items-center px-4" style={{ gap: '0.75rem', borderBottom: '1px solid var(--bulma-border-weak)', flexShrink: 0, minHeight: '3rem' }}>
        <span className="is-size-7 has-text-weight-semibold">Draft</span>
        {showTabs && <PlatformTabs platforms={platforms} selected={selectedPlatform} onSwitch={onTabSwitch} />}
      </div>
      <div style={{ flex: 1, overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: '0.75rem', padding: '0.75rem 1rem' }}>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
          {isHistory
            ? <div data-testid="post-text" className="is-size-7">{text}</div>
            : <textarea className="textarea is-size-7" aria-label="Post content"
                style={{ height: '220px', resize: 'none' }}
                value={text} onChange={(e) => onTextChange(e.target.value)} />}
          {isDraft(post) && post.status === 'failed' && post.error && (
            <p role="alert" className="is-size-7 has-text-danger mt-1">{post.error}</p>
          )}
          {saveError && <p role="alert" className="is-size-7 has-text-danger mt-1">{saveError}</p>}
        </div>
        {!isHistory && (
          <ImagePickers imageState={imageState} onCustomSet={onCustomSet} onUnsplashSelect={onUnsplashSelect} onRemove={onRemove} />
        )}
      </div>
      {!isHistory && (
        <div className="is-flex" style={{ gap: '0.5rem', padding: '0.75rem 1rem', borderTop: '1px solid var(--bulma-border-weak)', flexShrink: 0 }}>
          <button className="button is-small has-background-danger has-text-white" style={{ flex: 1, border: 'none' }} onClick={onDelete} aria-label="Delete">Delete</button>
          <button
            className={`button is-small ${isDirty ? 'has-background-warning has-text-white' : 'has-background-white-ter has-text-grey'}`}
            style={{ flex: 1, border: 'none' }} onClick={doSave} disabled={saveLoading}>Save</button>
        </div>
      )}
    </div>
  );
}
