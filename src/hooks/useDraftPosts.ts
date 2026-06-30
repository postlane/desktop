// SPDX-License-Identifier: BUSL-1.1

import type { DraftPost } from '../types'
import { useAsyncList } from './useAsyncList'

export interface DraftPostsState {
  drafts: DraftPost[]
  loading: boolean
  error: string | null
  refresh: () => void
  clear: () => void
}

export function useDraftPosts(): DraftPostsState {
  // v1: cross-repo load. Replace with get_org_drafts(project_id) when scale requires it.
  const { data: drafts, loading, error, refresh, clear } = useAsyncList<DraftPost>('get_all_drafts')
  return { drafts, loading, error, refresh, clear }
}
