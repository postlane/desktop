// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faChevronDown, faChevronRight } from '@fortawesome/free-solid-svg-icons';
import { useProjectsContext } from '../context/ProjectsProvider';
import { useDraftPostsContext } from '../context/DraftPostsProvider';
import { deriveOrgColour } from '../formatting/orgColour';
import { useNavPersistence } from '../hooks/useNavPersistence';
import type { Project, DraftPost, ViewSelection } from '../types';

interface Props {
  onNavigate: (_selection: ViewSelection) => void;
  onSettingsOpen: () => void;
  currentView: ViewSelection;
  onAddWorkspace?: () => void;
}

function useSettingsShortcut(onActivate: () => void) {
  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === ',') { e.preventDefault(); onActivate(); }
    };
    document.addEventListener('keydown', h);
    return () => document.removeEventListener('keydown', h);
  }, [onActivate]);
}

function countOrgDrafts(drafts: DraftPost[], projectId: string): number {
  return new Set(
    drafts
      .filter((d) => d.project_id === projectId)
      .map((d) => JSON.stringify([d.repo_path, d.post_folder])),
  ).size;
}

function AddOrgButton({ onClick }: { onClick?: () => void }) {
  return (
    <button className="button is-small is-light" onClick={onClick}>+ Add org</button>
  );
}

function OrgSubNav({ projectId, currentView, onNavigate, queueBadge }: {
  projectId: string; currentView: ViewSelection; onNavigate: (_sel: ViewSelection) => void;
  queueBadge: number;
}) {
  const isQueue = currentView.view === 'org_queue' && currentView.projectId === projectId;
  const isHistory = currentView.view === 'org_history' && currentView.projectId === projectId;
  const isSettings = currentView.view === 'org_settings' && currentView.projectId === projectId;
  function cls(active: boolean) {
    return 'button is-ghost is-small is-fullwidth is-justify-content-flex-start' +
      (active ? ' has-text-link has-text-weight-medium' : '');
  }
  return (
    <div style={{ marginLeft: '2.5rem', display: 'flex', flexDirection: 'column' }}>
      <button className={cls(isQueue)} aria-current={isQueue ? 'page' : undefined}
        onClick={() => onNavigate({ view: 'org_queue', projectId })}>
        Queue
        {queueBadge > 0 && <span className="tag is-info is-light is-rounded is-size-7" style={{ marginLeft: 'auto' }}>{queueBadge}</span>}
      </button>
      <button className={cls(isHistory)} aria-current={isHistory ? 'page' : undefined}
        onClick={() => onNavigate({ view: 'org_history', projectId })}>History</button>
      <button className={cls(isSettings)} aria-current={isSettings ? 'page' : undefined}
        onClick={() => onNavigate({ view: 'org_settings', projectId, section: 'queue' })}>Settings</button>
    </div>
  );
}

function OrgItem({ org, drafts, isExpanded, currentView, onToggle, onNavigate }: {
  org: Project; drafts: DraftPost[]; isExpanded: boolean; currentView: ViewSelection;
  onToggle: () => void; onNavigate: (_sel: ViewSelection) => void;
}) {
  const badge = countOrgDrafts(drafts, org.id);
  const colour = deriveOrgColour(org.id);
  return (
    <div className="px-3 py-1">
      <button onClick={onToggle} aria-expanded={isExpanded}
        className="button is-ghost is-small is-fullwidth is-justify-content-flex-start" style={{ gap: '0.5rem' }}>
        <span aria-label={`Workspace avatar for ${org.name}`} style={{
          width: 20, height: 20, borderRadius: '50%', background: colour,
          flexShrink: 0, display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
          color: '#fff', fontSize: '0.65rem', fontWeight: 700,
        }}>{org.name[0]?.toUpperCase()}</span>
        <FontAwesomeIcon icon={isExpanded ? faChevronDown : faChevronRight} className="is-size-7" style={{ flexShrink: 0 }} />
        <span className="is-size-7" style={{ flex: 1, textAlign: 'left', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{org.name}</span>
        {!isExpanded && badge > 0 && <span className="tag is-info is-light is-rounded is-size-7">{badge}</span>}
      </button>
      {isExpanded && <OrgSubNav projectId={org.id} currentView={currentView} onNavigate={onNavigate} queueBadge={badge} />}
    </div>
  );
}

function NavBrand() {
  return (
    <div className="is-flex is-align-items-center px-5 py-4" style={{ flexShrink: 0 }}>
      <span className="has-text-weight-semibold is-size-6" style={{ color: '#0f0f0f' }}>post</span>
      <span className="has-text-weight-semibold is-size-6 has-text-link">lane</span>
    </div>
  );
}

function SettingsFooter({ currentView, onNavigate }: {
  currentView: ViewSelection; onNavigate: (_sel: ViewSelection) => void;
}) {
  function cls(section: 'account' | 'preferences' | 'system') {
    const active = currentView.view === 'global_settings' && currentView.section === section;
    return 'button is-ghost is-small is-fullwidth is-justify-content-flex-start has-text-grey' +
      (active ? ' has-text-link' : '');
  }
  return (
    <div style={{ borderTop: '1px solid var(--bulma-border-weak)', padding: '0.5rem' }}>
      <button className={cls('account')} aria-label="Account settings"
        onClick={() => onNavigate({ view: 'global_settings', section: 'account' })}>Account</button>
      <button className={cls('preferences')} aria-label="Preferences settings"
        onClick={() => onNavigate({ view: 'global_settings', section: 'preferences' })}>Preferences</button>
      <button className={cls('system')} aria-label="System settings"
        onClick={() => onNavigate({ view: 'global_settings', section: 'system' })}>System</button>
    </div>
  );
}

export default function LeftNav({ onNavigate, onSettingsOpen, currentView, onAddWorkspace }: Props) {
  const { projects, loading, error, refresh } = useProjectsContext();
  const { drafts } = useDraftPostsContext();
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const scheduleWrite = useNavPersistence();
  useSettingsShortcut(onSettingsOpen);

  function handleToggle(orgId: string) {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      next.has(orgId) ? next.delete(orgId) : next.add(orgId);
      scheduleWrite(next, currentView);
      return next;
    });
  }

  function handleNavigate(sel: ViewSelection) { onNavigate(sel); scheduleWrite(expandedIds, sel); }

  const noProjects = !loading && !error && projects.length === 0;
  const hasProjects = !loading && !error && projects.length > 0;

  return (
    <nav role="navigation" aria-label="Main navigation"
      className="has-background-white" style={{ width: 256, height: '100vh', display: 'flex', flexDirection: 'column', borderRight: '1px solid var(--bulma-border-weak)' }}>
      <NavBrand />
      <div style={{ flex: 1, overflowY: 'auto', paddingBlock: '0.5rem' }}>
        {loading && <p className="is-size-7 has-text-grey px-4 py-2">Loading…</p>}
        {error && (
          <div className="px-4 py-2">
            <p className="is-size-7 has-text-danger">{error}</p>
            <button className="button is-small is-light mt-1" onClick={refresh} aria-label="Retry loading workspaces">Retry</button>
          </div>
        )}
        {noProjects && (
          <div className="px-4 py-2">
            <p className="is-size-7 has-text-grey">No workspaces yet.</p>
            <div className="mt-2"><AddOrgButton onClick={onAddWorkspace} /></div>
          </div>
        )}
        {projects.map((org) => (
          <OrgItem key={org.id} org={org} drafts={drafts}
            isExpanded={expandedIds.has(org.id)} currentView={currentView}
            onToggle={() => handleToggle(org.id)} onNavigate={handleNavigate} />
        ))}
        {hasProjects && <div className="px-3 py-1"><AddOrgButton onClick={onAddWorkspace} /></div>}
      </div>
      <SettingsFooter currentView={currentView} onNavigate={handleNavigate} />
    </nav>
  );
}
