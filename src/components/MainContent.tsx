// SPDX-License-Identifier: BUSL-1.1

import { useEffect } from 'react';
import { useProjectsContext } from '../context/ProjectsProvider';
import { OrgQueueView, OrgHistoryView, OrgSettingsDispatch, GlobalSettingsDispatch } from './OrgViews';
import { LoadingView } from '../AppLoadingStates';
import type { ViewSelection } from '../types';

export interface MainContentProps {
  view: ViewSelection;
  onNavigate: (_sel: ViewSelection) => void;
  onToast: (_msg: string, _durationMs?: number) => void;
  onTimezoneChange: (_tz: string) => void;
  onRepoChange: () => void;
  onSignedOut: () => void;
  wizardNudgePending?: boolean;
  onWizardNudgeHandled?: () => void;
}

export function MainContent({
  view, onNavigate, onToast, onTimezoneChange, onRepoChange: _onRepoChange,
  onSignedOut, wizardNudgePending, onWizardNudgeHandled,
}: MainContentProps) {
  const { projects, loading: projectsLoading } = useProjectsContext();

  useEffect(() => {
    if (projectsLoading || projects.length === 0) return;
    if (view.view !== 'no_orgs') return;
    if (wizardNudgePending) {
      onNavigate({ view: 'org_settings', projectId: projects[0].id, section: 'queue' });
      onWizardNudgeHandled?.();
    } else {
      onNavigate({ view: 'org_queue', projectId: projects[0].id });
    }
  }, [wizardNudgePending, projects, projectsLoading, view, onNavigate, onWizardNudgeHandled]);

  if (view.view === 'org_queue') {
    return (
      <OrgQueueView projectId={view.projectId} onNavigate={onNavigate}
        onToast={onToast}
      />
    );
  }
  if (view.view === 'org_history') return <OrgHistoryView projectId={view.projectId} />;
  if (view.view === 'org_settings') return <OrgSettingsDispatch projectId={view.projectId} onNavigate={onNavigate} onToast={onToast} />;
  if (view.view === 'global_settings') {
    return (
      <GlobalSettingsDispatch section={view.section}
        onTimezoneChange={onTimezoneChange} onSignedOut={onSignedOut}
      />
    );
  }
  return <LoadingView />;
}
