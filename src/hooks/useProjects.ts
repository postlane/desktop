// SPDX-License-Identifier: BUSL-1.1

import type { Project } from '../types'
import { useAsyncList } from './useAsyncList'

export type { Project }

export interface ProjectsState {
  projects: Project[]
  loading: boolean
  error: string | null
  refresh: () => void
  clear: () => void
}

export function useProjects(): ProjectsState {
  const { data: projects, loading, error, refresh, clear } = useAsyncList<Project>('list_projects')
  return { projects, loading, error, refresh, clear }
}
