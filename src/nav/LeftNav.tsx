// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import type { MouseEvent } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faChevronDown, faChevronRight, faGear, faTriangleExclamation, faCircleMinus, faCirclePlus } from '@fortawesome/free-solid-svg-icons';
import { getRepoStatus, sortAndBucketRepos } from './navUtils';
import type { RepoWithStatus, ViewSelection, StatusIndicatorType } from '../types';
import { useRepoData } from '../hooks/useRepoData';
import { useAppStateRestore } from '../hooks/useAppStateRestore';
import { useMetaChangedListener } from '../hooks/useMetaChangedListener';
import { useWatcherHealth } from '../hooks/useWatcherHealth';
import { useNavPersistence } from '../hooks/useNavPersistence';

const DOT = { width: 8, height: 8, borderRadius: '50%', display: 'inline-block', flexShrink: 0 } as const;

interface Props {
  onNavigate: (_selection: ViewSelection) => void;
  onSettingsOpen: () => void;
  onAddRepo: () => void;
  onAddWorkspace?: () => void;
  currentView: ViewSelection;
  refreshKey?: number;
}

function StatusDot({ indicator, repoId, isStalled, onRestartWatcher }: {
  indicator: StatusIndicatorType; repoId: string; isStalled: boolean; onRestartWatcher: (_id: string) => void;
}) {
  if (indicator.type === 'warning') {
    return <FontAwesomeIcon icon={faTriangleExclamation} className="has-text-warning is-size-7" aria-label="Repo path not found" />;
  }
  if (indicator.type === 'none') return null;
  if (indicator.type === 'watching' && !isStalled) {
    return <span className="has-background-success" style={DOT} title="Watching for new drafts" aria-label="Watching for new drafts" />;
  }
  if (isStalled) {
    return (
      <button onClick={(e) => { e.stopPropagation(); onRestartWatcher(repoId); }}
        title="Watcher may have stalled — click to restart" aria-label="Watcher stalled, click to restart"
        className="has-background-warning" style={{ ...DOT, border: 'none', padding: 0, cursor: 'pointer' }} />
    );
  }
  if (indicator.type === 'single') {
    const bg = indicator.color === 'red' ? 'has-background-danger' : 'has-background-success';
    const label = indicator.color === 'red' ? 'Failed posts' : 'Ready posts';
    return <span className={bg} style={DOT} aria-label={label} />;
  }
  return (
    <span style={{ position: 'relative', width: 16, height: 8, flexShrink: 0, display: 'inline-block' }} aria-label="Ready and failed posts">
      <span className="has-background-danger" style={{ position: 'absolute', right: 8, width: 8, height: 8, borderRadius: '50%', zIndex: 10 }} />
      <span className="has-background-success" style={{ position: 'absolute', right: 0, width: 8, height: 8, borderRadius: '50%' }} />
    </span>
  );
}

function SubItems({ repo, currentView, onNavigate }: { repo: RepoWithStatus; currentView: ViewSelection; onNavigate: (_sel: ViewSelection) => void }) {
  function cls(section: ViewSelection['section']) {
    const active = currentView.repoId === repo.id && currentView.section === section;
    return 'button is-ghost is-small is-fullwidth is-justify-content-flex-start' + (active ? ' has-text-link has-text-weight-medium' : '');
  }
  return (
    <div style={{ marginLeft: '1.25rem', display: 'flex', flexDirection: 'column' }}>
      <button className={cls('drafts')} aria-current={currentView.repoId === repo.id && currentView.section === 'drafts' ? 'page' : undefined}
        onClick={() => onNavigate({ view: 'repo', repoId: repo.id, section: 'drafts' })}>Drafts</button>
      <button className={cls('published')} aria-current={currentView.repoId === repo.id && currentView.section === 'published' ? 'page' : undefined}
        onClick={() => onNavigate({ view: 'repo', repoId: repo.id, section: 'published' })}>Published</button>
    </div>
  );
}

function RepoRemoveButton({ confirming, name, onClick, onBlur }: { confirming: boolean; name: string; onClick: (_e: MouseEvent) => void; onBlur: () => void }) {
  return (
    <button onClick={onClick} onBlur={onBlur}
      aria-label={confirming ? `Confirm remove ${name}` : `Remove ${name}`}
      title={confirming ? 'Click again to confirm' : 'Remove repo'}
      className={'button is-ghost is-small' + (confirming ? ' has-text-danger' : ' has-text-grey-light')}>
      <FontAwesomeIcon icon={faCircleMinus} />
    </button>
  );
}

function RepoRow({ repo, isExpanded, isStalled, currentView, onToggle, onNavigate, onRestartWatcher, onRemove }: {
  repo: RepoWithStatus; isExpanded: boolean; isStalled: boolean; currentView: ViewSelection;
  onToggle: () => void; onNavigate: (_sel: ViewSelection) => void; onRestartWatcher: (_id: string) => void; onRemove: (_id: string) => void;
}) {
  const [hovered, setHovered] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const indicator = getRepoStatus(repo);
  const isDisabled = !repo.active;
  const isNotFound = !repo.path_exists;

  function handleRemoveClick(e: MouseEvent) {
    e.stopPropagation();
    if (confirming) { onRemove(repo.id); } else { setConfirming(true); }
  }

  function handleBlur() { setTimeout(() => setConfirming(false), 150); }

  const showRemove = hovered || confirming;
  const mouseProps = { onMouseEnter: () => setHovered(true), onMouseLeave: () => { setHovered(false); setConfirming(false); } };

  if (isDisabled || isNotFound) {
    return (
      <div className="px-3 py-1" {...mouseProps}>
        <div className="is-flex is-align-items-center" style={{ gap: '0.25rem' }}>
          <button onClick={() => onNavigate({ view: 'repo', repoId: repo.id, section: 'drafts' })}
            className={'button is-ghost is-small is-fullwidth is-justify-content-space-between' + (isDisabled ? ' has-text-grey-light' : '')}>
            <span className="is-size-7" style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{repo.name}</span>
            <StatusDot indicator={indicator} repoId={repo.id} isStalled={false} onRestartWatcher={onRestartWatcher} />
          </button>
          {showRemove && <RepoRemoveButton confirming={confirming} name={repo.name} onClick={handleRemoveClick} onBlur={handleBlur} />}
        </div>
      </div>
    );
  }

  return (
    <div className="px-3 py-1" {...mouseProps}>
      <div className="is-flex is-align-items-center" style={{ gap: '0.25rem' }}>
        <button onClick={onToggle} aria-expanded={isExpanded}
          className="button is-ghost is-small is-fullwidth is-justify-content-flex-start" style={{ gap: '0.5rem' }}>
          <FontAwesomeIcon icon={isExpanded ? faChevronDown : faChevronRight} className="is-size-7" style={{ flexShrink: 0 }} />
          <span className="is-size-7" style={{ flex: 1, textAlign: 'left', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{repo.name}</span>
          <StatusDot indicator={indicator} repoId={repo.id} isStalled={isStalled} onRestartWatcher={onRestartWatcher} />
        </button>
        {showRemove && <RepoRemoveButton confirming={confirming} name={repo.name} onClick={handleRemoveClick} onBlur={handleBlur} />}
      </div>
      {isExpanded && <SubItems repo={repo} currentView={currentView} onNavigate={onNavigate} />}
    </div>
  );
}

function AllReposRow({ isAllRepos, totalReady, totalFailed, currentView, onNavigate, onAddRepo }: {
  isAllRepos: boolean; totalReady: number; totalFailed: number; currentView: ViewSelection; onNavigate: (_sel: ViewSelection) => void; onAddRepo: () => void;
}) {
  return (
    <div className="px-3 py-1">
      <div className="is-flex is-align-items-center" style={{ gap: '0.25rem' }}>
        <button onClick={() => onNavigate({ view: 'all_repos', repoId: null, section: isAllRepos ? currentView.section : 'drafts' })}
          aria-current={isAllRepos ? 'page' : undefined}
          className={'button is-ghost is-small is-fullwidth is-justify-content-space-between has-text-weight-medium' + (isAllRepos ? ' has-text-link' : '')}>
          <span className="is-size-7">All repos</span>
          <span className="is-flex is-align-items-center" style={{ gap: '0.25rem' }}>
            {totalFailed > 0 && <span className="tag is-danger is-light is-rounded is-size-7">{totalFailed}</span>}
            {totalReady > 0 && <span className="tag is-success is-light is-rounded is-size-7">{totalReady}</span>}
          </span>
        </button>
        <button onClick={onAddRepo} aria-label="Add a repo" title="Add a repo" className="button is-ghost is-small has-text-grey-light">
          <FontAwesomeIcon icon={faCirclePlus} />
        </button>
      </div>
    </div>
  );
}

function NavBrand({ onAddWorkspace }: { onAddWorkspace?: () => void }) {
  return (
    <div className="is-flex is-align-items-center px-5 py-4" style={{ flexShrink: 0 }}>
      <span className="has-text-weight-semibold is-size-6" style={{ color: '#0f0f0f' }}>post</span>
      <span className="has-text-weight-semibold is-size-6 has-text-link">lane</span>
      {onAddWorkspace && (
        <button onClick={onAddWorkspace} aria-label="Add workspace" title="Add workspace"
          className="button is-ghost is-small has-text-grey-light ml-auto">
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 448 512" width="14" height="14" fill="currentColor" aria-hidden="true">
            <path d="M64 32C28.7 32 0 60.7 0 96L0 416c0 35.3 28.7 64 64 64l320 0c35.3 0 64-28.7 64-64l0-320c0-35.3-28.7-64-64-64L64 32zM200 344l0-64-64 0c-13.3 0-24-10.7-24-24s10.7-24 24-24l64 0 0-64c0-13.3 10.7-24 24-24s24 10.7 24 24l0 64 64 0c13.3 0 24 10.7 24 24s-10.7 24-24 24l-64 0 0 64c0 13.3-10.7 24-24 24s-24-10.7-24-24z"/>
          </svg>
        </button>
      )}
    </div>
  );
}

export default function LeftNav({ onNavigate, onSettingsOpen, onAddRepo, onAddWorkspace, currentView, refreshKey }: Props) {
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const { repos, loadError, refresh } = useRepoData();

  useEffect(() => { if (refreshKey !== undefined) refresh(); }, [refreshKey, refresh]);
  useAppStateRestore(repos, setExpandedIds, onNavigate);
  const lastWatcherEvent = useMetaChangedListener(refresh);
  const stalledRepos = useWatcherHealth(repos, lastWatcherEvent);
  const scheduleWrite = useNavPersistence();

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === ',') { e.preventDefault(); onSettingsOpen(); }
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [onSettingsOpen]);

  function handleToggle(repoId: string) {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      next.has(repoId) ? next.delete(repoId) : next.add(repoId);
      scheduleWrite(next, currentView);
      return next;
    });
  }

  function handleNavigate(sel: ViewSelection) { onNavigate(sel); scheduleWrite(expandedIds, sel); }

  async function handleRestartWatcher(repoId: string) {
    try { await invoke('set_repo_active', { id: repoId, active: false }); await invoke('set_repo_active', { id: repoId, active: true }); }
    catch (e) { console.error('Failed to restart watcher:', e); }
  }

  async function handleRemoveRepo(repoId: string) {
    try { await invoke('remove_repo', { id: repoId }); refresh(); }
    catch (e) { console.error('Failed to remove repo:', e); }
  }

  const { active, inactive } = sortAndBucketRepos(repos);
  const totalReady = repos.reduce((sum, r) => sum + r.ready_count, 0);
  const totalFailed = repos.reduce((sum, r) => sum + r.failed_count, 0);
  const isAllRepos = currentView.view === 'all_repos';

  return (
    <nav role="navigation" aria-label="Main navigation"
      className="has-background-white" style={{ width: 256, height: '100vh', display: 'flex', flexDirection: 'column', borderRight: '1px solid var(--bulma-border-weak)' }}>
      <NavBrand onAddWorkspace={onAddWorkspace} />
      <div style={{ flex: 1, overflowY: 'auto', paddingBlock: '0.5rem' }}>
        {loadError && <p className="has-text-danger is-size-7 px-4 py-2">{loadError}</p>}
        <AllReposRow isAllRepos={isAllRepos} totalReady={totalReady} totalFailed={totalFailed} currentView={currentView} onNavigate={handleNavigate} onAddRepo={onAddRepo} />
        {active.map((repo) => <RepoRow key={repo.id} repo={repo} isExpanded={expandedIds.has(repo.id)} isStalled={stalledRepos.has(repo.id)} currentView={currentView} onToggle={() => handleToggle(repo.id)} onNavigate={handleNavigate} onRestartWatcher={handleRestartWatcher} onRemove={handleRemoveRepo} />)}
        {active.length > 0 && inactive.length > 0 && <hr className="my-2 mx-4" />}
        {inactive.map((repo) => <RepoRow key={repo.id} repo={repo} isExpanded={expandedIds.has(repo.id)} isStalled={false} currentView={currentView} onToggle={() => handleToggle(repo.id)} onNavigate={handleNavigate} onRestartWatcher={handleRestartWatcher} onRemove={handleRemoveRepo} />)}
      </div>
      <div style={{ borderTop: '1px solid var(--bulma-border-weak)', padding: '0.75rem' }}>
        <button onClick={onSettingsOpen} aria-label="Open settings" title="Settings"
          className="button is-ghost is-small is-fullwidth is-justify-content-flex-start has-text-grey" style={{ gap: '0.5rem' }}>
          <FontAwesomeIcon icon={faGear} />
          <span>Settings</span>
        </button>
      </div>
    </nav>
  );
}
