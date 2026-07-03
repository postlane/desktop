// SPDX-License-Identifier: BUSL-1.1

import { useCallback } from 'react';
import { MantineProvider } from '@mantine/core';
import { postlaneTheme } from './theme';
import { TimezoneContext } from './TimezoneContext';
import { ProjectsProvider } from './context/ProjectsProvider';
import { DraftPostsProvider } from './context/DraftPostsProvider';
import { useAppState } from './hooks/useAppState';
import { useToast, useCmdHShortcut, useWindowSizePersistence, useDirtyNavGuard } from './hooks/useAppHooks';
import { AppShell } from './AppShell';
import Wizard from './wizard/Wizard';
import WizardResumePrompt from './wizard/WizardResumePrompt';
import ReSignInScreen from './wizard/ReSignInScreen';

export { MainContent } from './components/MainContent';
export type { MainContentProps } from './components/MainContent';

export default function App() {
  return (
    <MantineProvider theme={postlaneTheme}>
      <AppContent />
    </MantineProvider>
  );
}

// v2.0 checklist 24.0.2: existing Bulma-styled screens (Wizard, AppShell,
// etc.) are untouched here -- MantineProvider only supplies context/CSS
// variables for the Mantine components built from this release onward. No
// existing markup is migrated as part of this item.
function AppContent() {
  const appState = useAppState();
  const { toastMessage, showToast } = useToast();
  const guard = useDirtyNavGuard(appState.setCurrentView, appState.currentView);
  const { handleNavClick } = guard;
  const cmdHCallback = useCallback(() => {
    const view = appState.currentView;
    const projectId = (view.view === 'org_queue' || view.view === 'org_history' || view.view === 'org_settings')
      ? view.projectId : '';
    handleNavClick({ view: 'org_history', projectId });
  }, [appState.currentView, handleNavClick]);
  useCmdHShortcut(cmdHCallback);
  useWindowSizePersistence();

  if (appState.initError) return (
    <div className="is-flex is-align-items-center is-justify-content-center" style={{ height: '100vh' }}>
      <p role="alert" className="is-size-7 has-text-danger has-text-centered" style={{ maxWidth: '24rem' }}>
        Failed to start Postlane: {appState.initError}
      </p>
    </div>
  );

  if (appState.showWizard && appState.resumeStep) {
    return <WizardResumePrompt step={appState.resumeStep} onResume={() => appState.setResumeStep(null)} onStartOver={appState.handleResumeDecline} />;
  }
  if (appState.showWizard) return <Wizard startAt={appState.wizardStartStep} initialProvider={appState.wizardProvider} initialWorkspaceId={appState.wizardWorkspaceId} initialWorkspaceName={appState.wizardWorkspaceName} onComplete={appState.handleWizardComplete} />;
  if (appState.showReSignIn) return <ReSignInScreen onSignedIn={appState.handleSignedIn} />;

  return (
    <ProjectsProvider>
    <DraftPostsProvider>
    <TimezoneContext.Provider value={appState.timezone}>
      <AppShell appState={appState} guard={guard} showToast={showToast} toastMessage={toastMessage} />
    </TimezoneContext.Provider>
    </DraftPostsProvider>
    </ProjectsProvider>
  );
}
