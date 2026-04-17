// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  ChevronDownIcon,
  ChevronRightIcon,
  Cog6ToothIcon,
  ExclamationTriangleIcon,
  MinusCircleIcon,
  PlusCircleIcon,
} from '@heroicons/react/24/outline';
import { getRepoStatus, sortAndBucketRepos } from './navUtils';
import type {
  RepoWithStatus,
  AppStateFile,
  ViewSelection,
  StatusIndicatorType,
  MetaChangedPayload,
} from '../types';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const WATCHER_STALE_MS = 60_000;
const PERSIST_DEBOUNCE_MS = 300;
const HEALTH_TICK_MS = 10_000;
const ACTIVITY_WINDOW_MS = 24 * 60 * 60 * 1000;

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface Props {
  onNavigate: (selection: ViewSelection) => void;
  onSettingsOpen: () => void;
  onAddRepo: () => void;
  currentView: ViewSelection;
  refreshKey?: number;
}

// ---------------------------------------------------------------------------
// Hooks
// ---------------------------------------------------------------------------

function useRepoData() {
  const [repos, setRepos] = useState<RepoWithStatus[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const updated = await invoke<RepoWithStatus[]>('get_repos');
      setRepos(updated);
    } catch (e) {
      console.error('Failed to refresh repos:', e);
    }
  }, []);

  useEffect(() => {
    invoke<RepoWithStatus[]>('get_repos')
      .then(setRepos)
      .catch((e) => {
        setLoadError('Could not load repositories. Check logs.');
        console.error('Failed to load repos:', e);
      });
  }, []);

  return { repos, loadError, refresh };
}

function useAppStateRestore(
  repos: RepoWithStatus[],
  setExpandedIds: React.Dispatch<React.SetStateAction<Set<string>>>,
  onNavigate: (sel: ViewSelection) => void,
) {
  useEffect(() => {
    if (repos.length === 0) return;
    invoke<AppStateFile>('read_app_state_command')
      .then((appState) => {
        const validIds = appState.nav.expanded_repos.filter((id) =>
          repos.some((r) => r.id === id),
        );
        setExpandedIds(new Set(validIds));

        const lastRepoId = appState.nav.last_repo_id;
        if (lastRepoId && repos.some((r) => r.id === lastRepoId)) {
          onNavigate({
            view: appState.nav.last_view as ViewSelection['view'],
            repoId: lastRepoId,
            section: appState.nav.last_section as ViewSelection['section'],
          });
          setExpandedIds((prev) => new Set([...prev, lastRepoId]));
        }
      })
      .catch(() => {
        // Silently default to empty state on missing/corrupt app_state.json
      });
    // Only restore once when repos first load
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repos.length > 0]);
}

function useMetaChangedListener(onRefresh: () => void) {
  const [lastWatcherEvent, setLastWatcherEvent] = useState<Map<string, Date>>(
    new Map(),
  );

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<MetaChangedPayload>('meta-changed', (event) => {
      setLastWatcherEvent((prev) => {
        const next = new Map(prev);
        next.set(event.payload.repo_id, new Date());
        return next;
      });
      onRefresh();
    })
      .then((fn) => { unlisten = fn; })
      .catch(console.error);
    return () => { unlisten?.(); };
  }, [onRefresh]);

  return lastWatcherEvent;
}

function useWatcherHealth(
  repos: RepoWithStatus[],
  lastWatcherEvent: Map<string, Date>,
): Set<string> {
  const [stalledRepos, setStalledRepos] = useState<Set<string>>(new Set());

  useEffect(() => {
    const tick = () => {
      const now = Date.now();
      const stalled = new Set<string>();
      for (const repo of repos) {
        if (!repo.active || !repo.path_exists) continue;
        const lastEvent = lastWatcherEvent.get(repo.id);
        const elapsed = lastEvent ? now - lastEvent.getTime() : Infinity;
        const recentActivity =
          repo.last_post_at !== null &&
          now - new Date(repo.last_post_at).getTime() < ACTIVITY_WINDOW_MS;
        if (elapsed > WATCHER_STALE_MS && recentActivity) {
          stalled.add(repo.id);
        }
      }
      setStalledRepos(stalled);
    };

    const id = setInterval(tick, HEALTH_TICK_MS);
    tick(); // run immediately on mount / dep change
    return () => clearInterval(id);
  }, [repos, lastWatcherEvent]);

  return stalledRepos;
}

// ---------------------------------------------------------------------------
// Sub-renderers
// ---------------------------------------------------------------------------

function StatusDot({
  indicator,
  repoId,
  isStalled,
  onRestartWatcher,
}: {
  indicator: StatusIndicatorType;
  repoId: string;
  isStalled: boolean;
  onRestartWatcher: (id: string) => void;
}) {
  if (indicator.type === 'warning') {
    return (
      <ExclamationTriangleIcon
        className="h-4 w-4 shrink-0 text-yellow-500"
        aria-label="Repo path not found"
      />
    );
  }
  if (indicator.type === 'none') return null;

  if (indicator.type === 'watching' && !isStalled) {
    return (
      <span
        className="h-2 w-2 shrink-0 rounded-full bg-green-500"
        title="Watching for new drafts"
        aria-label="Watching for new drafts"
      />
    );
  }

  if (isStalled) {
    return (
      <button
        onClick={(e) => { e.stopPropagation(); onRestartWatcher(repoId); }}
        title="Watcher may have stalled — click to restart"
        aria-label="Watcher stalled, click to restart"
        className="h-2 w-2 shrink-0 rounded-full bg-amber-500"
      />
    );
  }

  if (indicator.type === 'single') {
    const bg = indicator.color === 'red' ? 'bg-red-500' : 'bg-green-500';
    const label = indicator.color === 'red' ? 'Failed posts' : 'Ready posts';
    return (
      <span
        className={`h-2 w-2 shrink-0 rounded-full ${bg}`}
        title={indicator.color === 'green' ? 'Watching for new drafts' : undefined}
        aria-label={label}
      />
    );
  }

  // stacked: red on top/left, green offset right
  return (
    <span className="relative h-2 w-4 shrink-0" aria-label="Ready and failed posts">
      <span className="absolute right-2 h-2 w-2 rounded-full bg-red-500 z-10" />
      <span className="absolute right-0 h-2 w-2 rounded-full bg-green-500 z-0" />
    </span>
  );
}

function SubItems({
  repo,
  currentView,
  onNavigate,
}: {
  repo: RepoWithStatus;
  currentView: ViewSelection;
  onNavigate: (sel: ViewSelection) => void;
}) {
  const itemClass = (section: ViewSelection['section']) => {
    const isCurrent =
      currentView.repoId === repo.id && currentView.section === section;
    return [
      'w-full rounded-md px-3 py-1.5 text-left text-sm',
      'focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500',
      isCurrent
        ? 'bg-zinc-100 font-medium text-zinc-900 dark:bg-zinc-800 dark:text-zinc-100'
        : 'text-zinc-600 hover:bg-zinc-50 dark:text-zinc-400 dark:hover:bg-zinc-800/50',
    ].join(' ');
  };

  return (
    <div className="ml-5 flex flex-col">
      <button
        className={itemClass('drafts')}
        aria-current={
          currentView.repoId === repo.id && currentView.section === 'drafts'
            ? 'page'
            : undefined
        }
        onClick={() =>
          onNavigate({ view: 'repo', repoId: repo.id, section: 'drafts' })
        }
      >
        Drafts
      </button>
      <button
        className={itemClass('published')}
        aria-current={
          currentView.repoId === repo.id && currentView.section === 'published'
            ? 'page'
            : undefined
        }
        onClick={() =>
          onNavigate({ view: 'repo', repoId: repo.id, section: 'published' })
        }
      >
        Published
      </button>
    </div>
  );
}

function RepoRow({
  repo,
  isExpanded,
  isStalled,
  currentView,
  onToggle,
  onNavigate,
  onRestartWatcher,
  onRemove,
}: {
  repo: RepoWithStatus;
  isExpanded: boolean;
  isStalled: boolean;
  currentView: ViewSelection;
  onToggle: () => void;
  onNavigate: (sel: ViewSelection) => void;
  onRestartWatcher: (id: string) => void;
  onRemove: (id: string) => void;
}) {
  const [hovered, setHovered] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const indicator = getRepoStatus(repo);
  const isDisabled = !repo.active;
  const isNotFound = !repo.path_exists;

  function handleRemoveClick(e: React.MouseEvent) {
    e.stopPropagation();
    if (confirming) {
      onRemove(repo.id);
    } else {
      setConfirming(true);
    }
  }

  function handleBlur() {
    // Cancel confirm if user tabs away
    setTimeout(() => setConfirming(false), 150);
  }

  const removeButton = (
    <button
      onClick={handleRemoveClick}
      onBlur={handleBlur}
      aria-label={confirming ? `Confirm remove ${repo.name}` : `Remove ${repo.name}`}
      title={confirming ? 'Click again to confirm' : 'Remove repo'}
      className={[
        'shrink-0 rounded-md p-1 transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500',
        confirming
          ? 'text-red-500'
          : 'text-zinc-400 hover:text-red-400 dark:text-zinc-400 dark:hover:text-red-400',
      ].join(' ')}
    >
      <MinusCircleIcon className="h-5 w-5" />
    </button>
  );

  if (isDisabled || isNotFound) {
    return (
      <div
        className="px-3 py-1"
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => { setHovered(false); setConfirming(false); }}
      >
        <div className="flex items-center gap-1">
          <button
            onClick={() => onNavigate({ view: 'repo', repoId: repo.id, section: 'drafts' })}
            className={[
              'flex flex-1 items-center justify-between rounded-md px-2 py-1.5 text-sm',
              'focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500',
              isDisabled ? 'text-zinc-400 dark:text-zinc-600' : 'text-zinc-700 dark:text-zinc-300',
            ].join(' ')}
          >
            <span className="truncate">{repo.name}</span>
            <StatusDot indicator={indicator} repoId={repo.id} isStalled={false} onRestartWatcher={onRestartWatcher} />
          </button>
          {(hovered || confirming) && removeButton}
        </div>
      </div>
    );
  }

  return (
    <div
      className="px-3 py-1"
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => { setHovered(false); setConfirming(false); }}
    >
      <div className="flex items-center gap-1">
        <button
          onClick={onToggle}
          aria-expanded={isExpanded}
          className="flex flex-1 items-center gap-2 rounded-md px-2 py-1.5 text-sm text-zinc-700 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
        >
          {isExpanded ? (
            <ChevronDownIcon className="h-3 w-3 shrink-0" />
          ) : (
            <ChevronRightIcon className="h-3 w-3 shrink-0" />
          )}
          <span className="flex-1 truncate text-left">{repo.name}</span>
          <StatusDot indicator={indicator} repoId={repo.id} isStalled={isStalled} onRestartWatcher={onRestartWatcher} />
        </button>
        {(hovered || confirming) && removeButton}
      </div>
      {isExpanded && (
        <SubItems repo={repo} currentView={currentView} onNavigate={onNavigate} />
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export default function LeftNav({ onNavigate, onSettingsOpen, onAddRepo, currentView, refreshKey }: Props) {
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const persistTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const { repos, loadError, refresh } = useRepoData();

  // Re-fetch whenever Settings adds or removes a repo.
  useEffect(() => { if (refreshKey !== undefined) refresh(); }, [refreshKey, refresh]);

  useAppStateRestore(repos, setExpandedIds, onNavigate);
  const lastWatcherEvent = useMetaChangedListener(refresh);
  const stalledRepos = useWatcherHealth(repos, lastWatcherEvent);

  // Keyboard shortcut: Cmd+, / Ctrl+,
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === ',') {
        e.preventDefault();
        onSettingsOpen();
      }
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [onSettingsOpen]);

  function scheduleWrite(ids: Set<string>, view: ViewSelection) {
    if (persistTimerRef.current) clearTimeout(persistTimerRef.current);
    persistTimerRef.current = setTimeout(async () => {
      try {
        const win = getCurrentWindow();
        const [size, pos] = await Promise.all([
          win.outerSize(),
          win.outerPosition(),
        ]);
        await invoke<void>('save_app_state_command', {
          state: {
            version: 1,
            window: { width: size.width, height: size.height, x: pos.x, y: pos.y },
            nav: {
              last_view: view.view,
              last_repo_id: view.repoId,
              last_section: view.section,
              expanded_repos: [...ids],
            },
          },
        });
      } catch (e) {
        console.error('Failed to persist nav state:', e);
      }
    }, PERSIST_DEBOUNCE_MS);
  }

  function handleToggle(repoId: string) {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      next.has(repoId) ? next.delete(repoId) : next.add(repoId);
      scheduleWrite(next, currentView);
      return next;
    });
  }

  function handleNavigate(sel: ViewSelection) {
    onNavigate(sel);
    scheduleWrite(expandedIds, sel);
  }

  async function handleRestartWatcher(repoId: string) {
    try {
      await invoke('set_repo_active', { id: repoId, active: false });
      await invoke('set_repo_active', { id: repoId, active: true });
    } catch (e) {
      console.error('Failed to restart watcher:', e);
    }
  }

  async function handleRemoveRepo(repoId: string) {
    try {
      await invoke('remove_repo', { id: repoId });
      refresh();
    } catch (e) {
      console.error('Failed to remove repo:', e);
    }
  }

  const { active, inactive } = sortAndBucketRepos(repos);
  const totalReady = repos.reduce((sum, r) => sum + r.ready_count, 0);
  const totalFailed = repos.reduce((sum, r) => sum + r.failed_count, 0);
  const isAllRepos = currentView.view === 'all_repos';

  return (
    <nav
      role="navigation"
      aria-label="Main navigation"
      className="flex h-screen w-64 flex-col border-r border-zinc-200 bg-white dark:border-zinc-800 dark:bg-zinc-900"
    >
      <div className="flex-1 overflow-y-auto py-2">
        {loadError && (
          <p className="px-4 py-2 text-sm text-red-500">{loadError}</p>
        )}

        {/* All repos */}
        <div className="px-3 py-1">
          <div className="flex items-center gap-1">
          <button
            onClick={() =>
              handleNavigate({
                view: 'all_repos',
                repoId: null,
                section: isAllRepos ? currentView.section : 'drafts',
              })
            }
            aria-current={isAllRepos ? 'page' : undefined}
            className={[
              'flex flex-1 items-center justify-between rounded-md px-2 py-1.5 text-sm font-medium',
              'focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500',
              isAllRepos
                ? 'bg-zinc-100 text-zinc-900 dark:bg-zinc-800 dark:text-zinc-100'
                : 'text-zinc-700 hover:bg-zinc-50 dark:text-zinc-300 dark:hover:bg-zinc-800/50',
            ].join(' ')}
          >
            <span>All repos</span>
            <span className="flex items-center gap-1">
              {totalFailed > 0 && (
                <span className="rounded-full bg-red-100 px-1.5 py-0.5 text-xs font-medium text-red-700 dark:bg-red-900/30 dark:text-red-400">
                  {totalFailed}
                </span>
              )}
              {totalReady > 0 && (
                <span className="rounded-full bg-green-100 px-1.5 py-0.5 text-xs font-medium text-green-700 dark:bg-green-900/30 dark:text-green-400">
                  {totalReady}
                </span>
              )}
            </span>
          </button>
          <button
            onClick={onAddRepo}
            aria-label="Add a repo"
            title="Add a repo"
            className="shrink-0 rounded-md p-1 text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
          >
            <PlusCircleIcon className="h-5 w-5" />
          </button>
          </div>
        </div>

        {/* Active repos */}
        {active.map((repo) => (
          <RepoRow
            key={repo.id}
            repo={repo}
            isExpanded={expandedIds.has(repo.id)}
            isStalled={stalledRepos.has(repo.id)}
            currentView={currentView}
            onToggle={() => handleToggle(repo.id)}
            onNavigate={handleNavigate}
            onRestartWatcher={handleRestartWatcher}
            onRemove={handleRemoveRepo}
          />
        ))}

        {/* Divider */}
        {active.length > 0 && inactive.length > 0 && (
          <div className="mx-4 my-2 border-t border-zinc-200 dark:border-zinc-700" />
        )}

        {/* Inactive repos */}
        {inactive.map((repo) => (
          <RepoRow
            key={repo.id}
            repo={repo}
            isExpanded={expandedIds.has(repo.id)}
            isStalled={false}
            currentView={currentView}
            onToggle={() => handleToggle(repo.id)}
            onNavigate={handleNavigate}
            onRestartWatcher={handleRestartWatcher}
            onRemove={handleRemoveRepo}
          />
        ))}
      </div>

      {/* Settings — anchored at bottom */}
      <div className="border-t border-zinc-200 p-3 dark:border-zinc-800">
        <button
          onClick={onSettingsOpen}
          aria-label="Open settings"
          title="Settings"
          className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
        >
          <Cog6ToothIcon className="h-5 w-5" />
          <span>Settings</span>
        </button>
      </div>
    </nav>
  );
}
