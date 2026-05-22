// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import type { DraftPost } from '../types';
import { isDirectImageUrl } from '../drafts/imageUrlUtils';

type Attribution = { photographer_name: string; photographer_url: string };

export function usePostCardImage(post: DraftPost) {
  const [imageUrl, setImageUrl] = useState<string | null>(post.image_url ?? null);
  const [addingImage, setAddingImage] = useState(false);
  const [imageInput, setImageInput] = useState('');
  const [fetchingOg, setFetchingOg] = useState(false);
  const [ogFetchError, setOgFetchError] = useState<string | null>(null);
  const [hasUnsplashKey, setHasUnsplashKey] = useState(false);

  useEffect(() => {
    if (addingImage) {
      invoke<boolean>('has_unsplash_key').then(setHasUnsplashKey).catch(() => setHasUnsplashKey(false));
    }
  }, [addingImage]);

  const openImageInput = useCallback(() => { setImageInput(imageUrl ?? ''); setAddingImage(true); setOgFetchError(null); }, [imageUrl]);
  const closeImageInput = useCallback(() => { setAddingImage(false); setImageInput(''); setOgFetchError(null); }, []);

  const handleSaveImage = useCallback(async (url: string) => {
    let resolvedUrl = url;
    if (!isDirectImageUrl(url)) {
      setFetchingOg(true); setOgFetchError(null);
      try {
        const found = await invoke<string | null>('fetch_og_image', { url });
        if (found) { resolvedUrl = found; }
        else { setOgFetchError('No image found on that page. Paste a direct image URL instead.'); setFetchingOg(false); return; }
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setOgFetchError(msg.startsWith('unreachable:') ? 'Could not reach this URL. Check the page is publicly accessible.' : msg);
        setFetchingOg(false); return;
      }
      setFetchingOg(false);
    }
    try {
      await invoke('update_post_image', { repoPath: post.repo_path, postFolder: post.post_folder, imageUrl: resolvedUrl });
      setImageUrl(resolvedUrl); setAddingImage(false); setImageInput(''); setOgFetchError(null);
    } catch (e) { console.error('update_post_image failed:', e); }
  }, [post]);

  const handleRemoveImage = useCallback(async () => {
    try {
      await invoke('update_post_image', { repoPath: post.repo_path, postFolder: post.post_folder, imageUrl: null });
      setImageUrl(null);
    } catch (e) { console.error('update_post_image failed:', e); }
  }, [post]);

  const handleSelectUnsplash = useCallback(async (url: string, dl: string, attr: Attribution) => {
    try {
      await invoke('update_post_image_unsplash', {
        repoPath: post.repo_path, postFolder: post.post_folder,
        imageUrl: url, downloadLocation: dl,
        photographerName: attr.photographer_name, photographerUrl: attr.photographer_url,
      });
      setImageUrl(url);
    } catch (e) { console.error('update_post_image_unsplash failed:', e); }
  }, [post]);

  return {
    imageUrl, addingImage, imageInput, fetchingOg, ogFetchError, hasUnsplashKey,
    openImageInput, closeImageInput, handleSaveImage, handleRemoveImage, handleSelectUnsplash,
    onInputChange: (v: string) => { setImageInput(v); setOgFetchError(null); },
  };
}
