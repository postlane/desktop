// SPDX-License-Identifier: BUSL-1.1

import XCard from './XCard';
import BlueskyCard from './BlueskyCard';
import MastodonCard from './MastodonCard';

type Platform = 'x' | 'bluesky' | 'mastodon';

interface PostPreviewProps {
  content?: string;
  platform?: Platform;
  imageUrl?: string;
  onSave?: (newContent: string) => void;
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
  return <XCard {...sharedProps} />;
}
