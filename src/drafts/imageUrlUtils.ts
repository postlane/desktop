// SPDX-License-Identifier: BUSL-1.1

const IMAGE_CDN_HOSTNAMES = new Set([
  'images.unsplash.com', 'cdn.pixabay.com', 'images.pexels.com',
  'lh3.googleusercontent.com', 'pbs.twimg.com', 'media.giphy.com',
]);
const IMAGE_EXTENSIONS = ['jpg', 'jpeg', 'png', 'webp', 'gif', 'avif', 'svg'];

export function isDirectImageUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    if (IMAGE_CDN_HOSTNAMES.has(parsed.hostname)) return true;
    return IMAGE_EXTENSIONS.some((ext) => parsed.pathname.toLowerCase().endsWith(`.${ext}`));
  } catch { return false; }
}
