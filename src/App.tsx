// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from './ipc/invoke';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import Wizard from './wizard/Wizard';
import ReSignInScreen from './wizard/ReSignInScreen';
import AddWorkspaceModal from './wizard/AddWorkspaceModal';
import TelemetryConsentModal from './telemetry/TelemetryConsentModal';
import LeftNav from './nav/LeftNav';
import PostTable from './components/PostTable';
import EditPostView from './components/EditPostView';
import OrgSettingsView from './settings/OrgSettingsView';
import AccountSettingsView from './settings/AccountSettingsView';
import PreferencesSettingsView from './settings/PreferencesSettingsView';
import SystemSettingsView from './settings/SystemSettingsView';
import { TimezoneContext, useTimezone } from './TimezoneContext';
import { ProjectsProvider, useProjectsContext } from './context/ProjectsProvider';
import { DraftPostsProvider, useDraftPostsContext } from './context/DraftPostsProvider';
import { useSentPosts } from './hooks/useSentPosts';
import type { AppStateFile, ViewSelection, DraftPost } from './types';

const DEFAULT_VIEW: ViewSelection = { view: 'no_orgs' };

// ── MainContent sub-components ────────────────────────────────────────────────

interface OrgQueueViewProps {
  projectId: string;
  onNavigate: (_sel: ViewSelection) => void;
  onToast: (_msg: string, _durationMs?: number) => void;
  onDirtyChange: (_dirty: boolean) => void;
  pendingNavSel?: ViewSelection | null;
  onNavCancelled?: () => void;
}

export interface MainContentProps {
  view: ViewSelection;
  onNavigate: (_sel: ViewSelection) => void;
  onToast: (_msg: string, _durationMs?: number) => void;
  onDirtyChange: (_dirty: boolean) => void;
  onTimezoneChange: (_tz: string) => void;
  onRepoChange: () => void;
  onSignedOut: () => void;
  pendingNavSel?: ViewSelection | null;
  onNavCancelled?: () => void;
  wizardNudgePending?: boolean;
  onWizardNudgeHandled?: () => void;
}

function LoadingView() {
  return (
    <div className="is-flex is-align-items-center is-justify-content-center" style={{ height: '100%' }}>
      <p className="is-size-7 has-text-grey">Loading…</p>
    </div>
  );
}

function QueueLoadError({ error, onRetry }: { error: string; onRetry: () => void }) {
  return (
    <div className="p-5">
      <p className="is-size-7 has-text-danger mb-3">{error}</p>
      <button className="button is-small" onClick={onRetry}>Retry</button>
    </div>
  );
}

function OrgQueueView({ projectId, onNavigate, onToast, onDirtyChange, pendingNavSel, onNavCancelled }: OrgQueueViewProps) {
  const [selectedPost, setSelectedPost] = useState<DraftPost | null>(null);
  const tz = useTimezone();
  const { drafts, loading, error, refresh } = useDraftPostsContext();
  const { projects } = useProjectsContext();
  const project = projects.find(p => p.id === projectId) ?? null;
  const projectDrafts = drafts.filter(d => d.project_id === projectId);

  if (selectedPost && project) {
    return (
      <EditPostView post={selectedPost} project={project} isHistory={false} timezone={tz}
        onBack={() => { setSelectedPost(null); onDirtyChange(false); }}
        onApproved={() => { setSelectedPost(null); refresh(); onDirtyChange(false); }}
        onToast={onToast} onNavigate={onNavigate} onDirtyChange={onDirtyChange}
        pendingNavSel={pendingNavSel} onNavCancelled={onNavCancelled}
      />
    );
  }
  if (loading) return <LoadingView />;
  if (error) return <QueueLoadError error={error} onRetry={refresh} />;
  return <PostTable posts={projectDrafts} isHistory={false} onSelect={setSelectedPost} timezone={tz} />;
}

function OrgHistoryView({ projectId }: { projectId: string }) {
  const tz = useTimezone();
  const { posts, loading, error, refresh } = useSentPosts(projectId);
  if (loading) return <LoadingView />;
  if (error) return <QueueLoadError error={error} onRetry={refresh} />;
  return <PostTable posts={posts} isHistory={true} onSelect={() => {}} timezone={tz} />;
}

function OrgSettingsDispatch({ projectId }: { projectId: string }) {
  const { projects } = useProjectsContext();
  const project = projects.find(p => p.id === projectId);
  if (!project) return <LoadingView />;
  return <OrgSettingsView org={project} />;
}

function GlobalSettingsDispatch({ section, onTimezoneChange, onSignedOut }: {
  section: string; onTimezoneChange: (_tz: string) => void; onSignedOut: () => void;
}) {
  if (section === 'account') return <AccountSettingsView onSignedOut={onSignedOut} />;
  if (section === 'preferences') return <PreferencesSettingsView onTimezoneChange={onTimezoneChange} />;
  return <SystemSettingsView />;
}

export function MainContent({
  view, onNavigate, onToast, onDirtyChange, onTimezoneChange, onRepoChange: _onRepoChange,
  onSignedOut, pendingNavSel, onNavCancelled, wizardNudgePending, onWizardNudgeHandled,
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
        onToast={onToast} onDirtyChange={onDirtyChange}
        pendingNavSel={pendingNavSel} onNavCancelled={onNavCancelled}
      />
    );
  }
  if (view.view === 'org_history') return <OrgHistoryView projectId={view.projectId} />;
  if (view.view === 'org_settings') return <OrgSettingsDispatch projectId={view.projectId} />;
  if (view.view === 'global_settings') {
    return (
      <GlobalSettingsDispatch section={view.section}
        onTimezoneChange={onTimezoneChange} onSignedOut={onSignedOut}
      />
    );
  }
  return <LoadingView />;
}

// ── Window size persistence ───────────────────────────────────────────────────

function useWindowSizePersistence() {
  const resizeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    const win = getCurrentWindow();
    let unlisten: (() => void) | undefined;
    win.onResized(async ({ payload: size }) => {
      if (resizeTimerRef.current) clearTimeout(resizeTimerRef.current);
      resizeTimerRef.current = setTimeout(async () => {
        try {
          const pos = await win.outerPosition();
          const appState = await invoke<AppStateFile>('read_app_state_command');
          await invoke('save_app_state_command', {
            state: { ...appState, window: { width: size.width, height: size.height, x: pos.x, y: pos.y } },
          });
        } catch (e) { console.error('Failed to persist window size:', e instanceof Error ? e.message : String(e)); }
      }, 500);
    }).then((fn) => { unlisten = fn; }).catch((e: unknown) => console.error('Failed to set up resize listener:', e instanceof Error ? e.message : String(e)));
    return () => { unlisten?.(); if (resizeTimerRef.current) clearTimeout(resizeTimerRef.current); };
  }, []);
}

function useCmdHShortcut(onActivate: () => void) {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'h') { e.preventDefault(); onActivate(); }
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [onActivate]);
}

// ── Toast hook ────────────────────────────────────────────────────────────────

function useToast() {
  const [toastMessage, setToastMessage] = useState<string | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const showToast = useCallback((msg: string) => {
    if (timerRef.current) clearTimeout(timerRef.current);
    setToastMessage(msg);
    timerRef.current = setTimeout(() => setToastMessage(null), 3000);
  }, []);

  return { toastMessage, showToast };
}

// ── Dirty nav guard ───────────────────────────────────────────────────────────

function useDirtyNavGuard(setCurrentView: (_sel: ViewSelection) => void) {
  const editPostViewDirtyRef = useRef(false);
  const [pendingNavigation, setPendingNavigation] = useState<ViewSelection | null>(null);
  const [discardModalOpen, setDiscardModalOpen] = useState(false);

  const handleNavClick = useCallback((sel: ViewSelection) => {
    if (editPostViewDirtyRef.current) {
      setPendingNavigation(sel);
      setDiscardModalOpen(true);
    } else {
      setCurrentView(sel);
    }
  }, [setCurrentView]);

  const confirmDiscard = useCallback(() => {
    if (pendingNavigation) setCurrentView(pendingNavigation);
    setPendingNavigation(null);
    setDiscardModalOpen(false);
    editPostViewDirtyRef.current = false;
  }, [pendingNavigation, setCurrentView]);

  const cancelDiscard = useCallback(() => {
    setPendingNavigation(null);
    setDiscardModalOpen(false);
  }, []);

  return { editPostViewDirtyRef, discardModalOpen, handleNavClick, confirmDiscard, cancelDiscard };
}

// ── App state ─────────────────────────────────────────────────────────────────

function useAppState() {
  const [currentView, setCurrentView] = useState<ViewSelection>(DEFAULT_VIEW);
  const [showWizard, setShowWizard] = useState(false);
  const [showReSignIn, setShowReSignIn] = useState(false);
  const [showAddWorkspace, setShowAddWorkspace] = useState(false);
  const [showConsentModal, setShowConsentModal] = useState(false);
  const [timezone, setTimezone] = useState<string>(() => Intl.DateTimeFormat().resolvedOptions().timeZone);
  const [repoVersion, setRepoVersion] = useState(0);
  const [initError, setInitError] = useState<string | null>(null);
  const [wizardNudgePending, setWizardNudgePending] = useState(false);
  const appStateRef = useRef<AppStateFile | null>(null);

  useEffect(() => {
    const unlisten = listen('license:expired', () => setShowReSignIn(true));
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  useEffect(() => {
    Promise.all([
      invoke<AppStateFile>('read_app_state_command'),
      invoke<boolean>('get_license_signed_in'),
    ])
      .then(([appState, hasToken]) => {
        appStateRef.current = appState;
        const tz = appState.timezone || Intl.DateTimeFormat().resolvedOptions().timeZone;
        setTimezone(tz);
        if (!appState.timezone) {
          invoke('save_app_state_command', { state: { ...appState, timezone: tz } }).catch(console.error);
        }
        if (!appState.consent_asked) setShowConsentModal(true);
        if (!appState.wizard_completed) { setShowWizard(true); return; }
        if (!hasToken) { setShowReSignIn(true); return; }
        if (!appState.post_wizard_completed) setWizardNudgePending(true);
      })
      .catch((e: unknown) => setInitError(e instanceof Error ? e.message : String(e)));
  }, []);

  function handleWizardComplete() { setShowWizard(false); }
  function handleSignedIn() { setShowReSignIn(false); }

  async function handleConsentChoice(consent: boolean) {
    try { await invoke('set_telemetry_consent', { consent }); }
    catch (e) { console.error('set_telemetry_consent failed:', e); }
    setShowConsentModal(false);
  }

  function handleSignedOut() { setShowReSignIn(true); }

  function handleWizardNudgeHandled() {
    setWizardNudgePending(false);
    if (appStateRef.current) {
      invoke('save_app_state_command', {
        state: { ...appStateRef.current, post_wizard_completed: true },
      }).catch(console.error);
    }
  }

  return {
    currentView, setCurrentView, showWizard, showReSignIn,
    showAddWorkspace, setShowAddWorkspace, showConsentModal,
    timezone, setTimezone, repoVersion, setRepoVersion,
    initError, wizardNudgePending,
    handleWizardComplete, handleSignedIn, handleConsentChoice, handleWizardNudgeHandled, handleSignedOut,
  };
}

// ── App shell (inner layout, inside providers) ────────────────────────────────

function AppShell({
  appState,
  guard,
  showToast,
  toastMessage,
}: {
  appState: ReturnType<typeof useAppState>;
  guard: ReturnType<typeof useDirtyNavGuard>;
  showToast: (_msg: string) => void;
  toastMessage: string | null;
}) {
  const { currentView, showConsentModal, showAddWorkspace,
    setShowAddWorkspace, setRepoVersion, wizardNudgePending,
    handleConsentChoice, handleWizardNudgeHandled, handleSignedOut, setTimezone } = appState;
  const { discardModalOpen, handleNavClick, confirmDiscard, cancelDiscard, editPostViewDirtyRef } = guard;
  return (
    <div className="is-flex" style={{ height: '100vh', overflow: 'hidden', background: 'white' }}>
      <LeftNav currentView={currentView} onNavigate={handleNavClick}
        onSettingsOpen={() => handleNavClick({ view: 'global_settings', section: 'account' })}
        onAddWorkspace={() => setShowAddWorkspace(true)} />
      {showConsentModal && <TelemetryConsentModal onAccept={() => handleConsentChoice(true)} onDecline={() => handleConsentChoice(false)} />}
      {showAddWorkspace && <AddWorkspaceModal onClose={() => setShowAddWorkspace(false)} onCreated={() => { setShowAddWorkspace(false); setRepoVersion((v) => v + 1); }} />}
      {discardModalOpen && (
        <div className="modal is-active">
          <div className="modal-background" onClick={cancelDiscard} />
          <div className="modal-content box has-text-centered" style={{ maxWidth: '20rem' }}>
            <p className="mb-4 is-size-7">You have unsaved changes. Discard them?</p>
            <div className="buttons is-centered">
              <button className="button is-danger is-small" onClick={confirmDiscard}>Discard</button>
              <button className="button is-small" onClick={cancelDiscard}>Cancel</button>
            </div>
          </div>
        </div>
      )}
      {toastMessage && (
        <div className="notification is-success is-small" style={{ position: 'fixed', bottom: '1rem', right: '1rem', zIndex: 9999 }}>
          {toastMessage}
        </div>
      )}
      <main style={{ flex: 1, overflowY: 'auto' }}>
        <MainContent key={JSON.stringify(currentView)} view={currentView}
          onNavigate={handleNavClick} onToast={showToast}
          onDirtyChange={(dirty) => { editPostViewDirtyRef.current = dirty; }}
          onTimezoneChange={setTimezone} onRepoChange={() => setRepoVersion((v) => v + 1)}
          onSignedOut={handleSignedOut} wizardNudgePending={wizardNudgePending}
          onWizardNudgeHandled={handleWizardNudgeHandled}
        />
      </main>
    </div>
  );
}

// ── Root component ────────────────────────────────────────────────────────────

export default function App() {
  const appState = useAppState();
  const { toastMessage, showToast } = useToast();
  const guard = useDirtyNavGuard(appState.setCurrentView);
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

  if (appState.showWizard) return <Wizard onComplete={appState.handleWizardComplete} />;
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
