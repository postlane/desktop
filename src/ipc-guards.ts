// SPDX-License-Identifier: BUSL-1.1

import type { DraftPost, PublishedPost } from './types';

function isObject(x: unknown): x is Record<string, unknown> {
  return typeof x === 'object' && x !== null && !Array.isArray(x);
}

export function isDraftPost(x: unknown): x is DraftPost {
  if (!isObject(x)) return false;
  if (typeof x.repo_id !== 'string') return false;
  if (typeof x.post_folder !== 'string') return false;
  if (x.status !== 'ready' && x.status !== 'failed') return false;
  if (!Array.isArray(x.platforms)) return false;
  return true;
}

export function isPublishedPost(x: unknown): x is PublishedPost {
  if (!isObject(x)) return false;
  if (typeof x.repo_id !== 'string') return false;
  if (typeof x.post_folder !== 'string') return false;
  if (x.status !== 'sent' && x.status !== 'queued') return false;
  if (!Array.isArray(x.platforms)) return false;
  return true;
}
