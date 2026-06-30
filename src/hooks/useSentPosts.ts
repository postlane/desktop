// SPDX-License-Identifier: BUSL-1.1

import type { PublishedPost } from '../types'
import { useAsyncList } from './useAsyncList'

export interface SentPostsState {
  posts: PublishedPost[]
  loading: boolean
  error: string | null
  refresh: () => void
}

export function useSentPosts(projectId: string): SentPostsState {
  const { data: posts, loading, error, refresh } = useAsyncList<PublishedPost>(
    'get_org_published',
    { projectId },
  )
  return { posts, loading, error, refresh }
}
