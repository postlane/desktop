// SPDX-License-Identifier: BUSL-1.1

import XCard from './XCard';
import BlueskyCard from './BlueskyCard';
import MastodonCard from './MastodonCard';
import LinkedInCard from './LinkedInCard';
import type { Platform } from '../types';

interface PostPreviewProps {
  content?: string;
  platform?: Platform;
  imageUrl?: string;
  onSave?: (_newContent: string) => void;
  onImageClick?: () => void;
  onApprove?: () => void;
  approveLabel?: string;
  onDelete?: () => void;
}

export default function PostPreview({
  content = '',
  platform = 'x',
  imageUrl,
  onSave,
  onImageClick,
  onApprove,
  approveLabel,
  onDelete,
}: PostPreviewProps) {
  const sharedProps = { content, imageUrl, onSave, onImageClick, onApprove, approveLabel, onDelete };
  if (platform === 'bluesky') return <BlueskyCard {...sharedProps} />;
  if (platform === 'mastodon') return <MastodonCard {...sharedProps} />;
  if (platform === 'linkedin') return <LinkedInCard {...sharedProps} />;
  return <XCard {...sharedProps} />;
}
