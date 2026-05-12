// SPDX-License-Identifier: BUSL-1.1

import type { CSSProperties } from 'react';

interface Props {
  size?: number;
  style?: CSSProperties;
}

export function UploadPostLogo({ size = 16, style }: Props) {
  return (
    <img
      src="https://upload-post.com/upload-post-cloud-transparent.png"
      alt=""
      width={size}
      height={size}
      aria-hidden="true"
      style={{ objectFit: 'contain', ...style }}
      onError={(e) => { e.currentTarget.style.display = 'none'; }}
    />
  );
}
