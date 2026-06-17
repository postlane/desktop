// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useMemo } from 'react';
import { useEditGuard } from '../context/EditGuardContext';
import { useTimezone } from '../TimezoneContext';
import { useDraftPostsContext } from '../context/DraftPostsProvider';
import { useProjectsContext } from '../context/ProjectsProvider';
import { useSentPosts } from '../hooks/useSentPosts';
import { useConnectedPlatforms } from '../hooks/useConnectedPlatforms';
import EditPostView from './EditPostView';
import PostTable from './PostTable';
import OrgUpgradeBanner from './OrgUpgradeBanner';
import OrgLinkModal from './OrgLinkModal';
import { MigrationBannersBlock } from '../settings/MigrationBanner';
import OrgSettingsView from '../settings/OrgSettingsView';
import AccountSettingsView from '../settings/AccountSettingsView';
import PreferencesSettingsView from '../settings/PreferencesSettingsView';
import SystemSettingsView from '../settings/SystemSettingsView';
import { LoadingView, QueueLoadError } from '../AppLoadingStates';
import type { ViewSelection, DraftPost, PublishedPost } from '../types';

export interface OrgQueueViewProps {
  projectId: string;
  onNavigate: (_sel: ViewSelection) => void;
  onToast: (_msg: string, _durationMs?: number) => void;
}

export function OrgQueueView({ projectId, onNavigate, onToast }: OrgQueueViewProps) {
  const { resetSignal, setDirty } = useEditGuard();
  const [selectedPost, setSelectedPost] = useState<DraftPost | null>(null);
  useEffect(() => { setSelectedPost(null); }, [resetSignal]);
  const [showOrgLink, setShowOrgLink] = useState(false);
  const tz = useTimezone();
  const { drafts, loading, error, refresh } = useDraftPostsContext();
  const { projects, refresh: refreshProjects } = useProjectsContext();
  const project = projects.find(p => p.id === projectId) ?? null;
  const projectDrafts = drafts.filter(d => d.project_id === projectId);
  const connectedPlatformsByRepo = useConnectedPlatforms(
    useMemo(() => [...new Set(projectDrafts.map(d => d.repo_id))], [projectDrafts]),
  );

  if (selectedPost && project) {
    return (
      <EditPostView post={selectedPost} project={project} isHistory={false} timezone={tz}
        onBack={() => { setSelectedPost(null); setDirty(false); }}
        onApproved={() => { setSelectedPost(null); refresh(); setDirty(false); }}
        onNavigate={onNavigate}
      />
    );
  }
  if (loading) return <LoadingView />;
  if (error) return <QueueLoadError error={error} onRetry={refresh} />;
  return (
    <>
      <MigrationBannersBlock projectId={projectId} />
      {project && <OrgUpgradeBanner project={project} onConnect={() => setShowOrgLink(true)} />}
      {showOrgLink && project && (
        <div className="modal is-active">
          <div className="modal-background" onClick={() => setShowOrgLink(false)} />
          <div className="modal-card">
            <header className="modal-card-head">
              <p className="modal-card-title is-size-6">Connect GitHub org</p>
            </header>
            <section className="modal-card-body">
              <OrgLinkModal
                projectId={project.id}
                onDone={(_orgLogin) => { setShowOrgLink(false); refreshProjects(); onToast('GitHub org connected.'); }}
                onClose={() => setShowOrgLink(false)}
              />
            </section>
          </div>
        </div>
      )}
      <PostTable posts={projectDrafts} isHistory={false} onSelect={setSelectedPost} timezone={tz}
        connectedPlatformsByRepo={connectedPlatformsByRepo}
        onConnectPlatform={() => onNavigate({ view: 'org_settings', projectId, section: 'settings' })} />
    </>
  );
}

export function OrgHistoryView({ projectId }: {
  projectId: string;
}) {
  const tz = useTimezone();
  const { posts, loading, error, refresh } = useSentPosts(projectId);
  const { projects } = useProjectsContext();
  const [selectedPost, setSelectedPost] = useState<PublishedPost | null>(null);
  const project = projects.find(p => p.id === projectId) ?? null;

  if (selectedPost && project) {
    return (
      <EditPostView post={selectedPost} project={project} isHistory={true} timezone={tz}
        onBack={() => setSelectedPost(null)}
        onApproved={() => setSelectedPost(null)}
        onNavigate={() => {}}
      />
    );
  }
  if (loading) return <LoadingView />;
  if (error) return <QueueLoadError error={error} onRetry={refresh} />;
  return <PostTable posts={posts} isHistory={true} onSelect={setSelectedPost} timezone={tz} />;
}

export function OrgSettingsDispatch({ projectId, onNavigate, onToast }: {
  projectId: string;
  onNavigate: (_sel: ViewSelection) => void;
  onToast: (_msg: string) => void;
}) {
  const { projects, refresh: refreshProjects } = useProjectsContext();
  const project = projects.find(p => p.id === projectId);
  if (!project) return <LoadingView />;
  function handleDisconnected() {
    refreshProjects();
    onNavigate({ view: 'no_orgs' });
    onToast('Workspace disconnected');
  }
  function handleDeleted() {
    refreshProjects();
    onNavigate({ view: 'no_orgs' });
    onToast('Workspace and all content deleted');
  }
  return <OrgSettingsView org={project} onDisconnected={handleDisconnected} onDeleted={handleDeleted} />;
}

export function GlobalSettingsDispatch({ section, onTimezoneChange, onSignedOut }: {
  section: string; onTimezoneChange: (_tz: string) => void; onSignedOut: () => void;
}) {
  if (section === 'account') return <AccountSettingsView onSignedOut={onSignedOut} />;
  if (section === 'preferences') return <PreferencesSettingsView onTimezoneChange={onTimezoneChange} />;
  return <SystemSettingsView />;
}
