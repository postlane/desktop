// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef, useCallback } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '../ipc/invoke';
import type { AppStateFile, ViewSelection } from '../types';

export function useWindowSizePersistence() {
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

export function useCmdHShortcut(onActivate: () => void) {
  const onActivateRef = useRef(onActivate);
  onActivateRef.current = onActivate;
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'h') { e.preventDefault(); onActivateRef.current(); }
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, []);
}

export function useToast() {
  const [toastMessage, setToastMessage] = useState<string | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const showToast = useCallback((msg: string) => {
    if (timerRef.current) clearTimeout(timerRef.current);
    setToastMessage(msg);
    timerRef.current = setTimeout(() => setToastMessage(null), 3000);
  }, []);

  return { toastMessage, showToast };
}

export function isSameView(a: ViewSelection, b: ViewSelection): boolean {
  if (a.view !== b.view) return false;
  if (a.view === 'org_queue' && b.view === 'org_queue') return a.projectId === b.projectId;
  if (a.view === 'org_history' && b.view === 'org_history') return a.projectId === b.projectId;
  if (a.view === 'org_settings' && b.view === 'org_settings') return a.projectId === b.projectId;
  if (a.view === 'global_settings' && b.view === 'global_settings') return a.section === b.section;
  return true;
}

type PendingAction =
  | { kind: 'nav'; sel: ViewSelection }
  | { kind: 'switch_account'; accountId: string };

export function useDirtyNavGuard(
  setCurrentView: (_sel: ViewSelection) => void,
  currentView: ViewSelection,
  onSwitchAccount: (_accountId: string) => void,
) {
  const editPostViewDirtyRef = useRef(false);
  const [pendingAction, setPendingAction] = useState<PendingAction | null>(null);
  const [discardModalOpen, setDiscardModalOpen] = useState(false);
  const [resetSignal, setResetSignal] = useState(0);

  const handleNavClick = useCallback((sel: ViewSelection) => {
    if (editPostViewDirtyRef.current) {
      setPendingAction({ kind: 'nav', sel });
      setDiscardModalOpen(true);
    } else if (isSameView(sel, currentView)) {
      setResetSignal((s) => s + 1);
    } else {
      setCurrentView(sel);
    }
  }, [setCurrentView, currentView]);

  // Switching provider accounts (checklist 24.4.10) shares the same guard as
  // nav: an unsaved approval-queue edit prompts to discard before switching,
  // rather than switching silently.
  const handleAccountSwitch = useCallback((accountId: string) => {
    if (editPostViewDirtyRef.current) {
      setPendingAction({ kind: 'switch_account', accountId });
      setDiscardModalOpen(true);
    } else {
      onSwitchAccount(accountId);
    }
  }, [onSwitchAccount]);

  const confirmDiscard = useCallback(() => {
    if (pendingAction?.kind === 'nav') {
      if (isSameView(pendingAction.sel, currentView)) {
        setResetSignal((s) => s + 1);
      } else {
        setCurrentView(pendingAction.sel);
      }
    } else if (pendingAction?.kind === 'switch_account') {
      onSwitchAccount(pendingAction.accountId);
    }
    setPendingAction(null);
    setDiscardModalOpen(false);
    editPostViewDirtyRef.current = false;
  }, [pendingAction, setCurrentView, currentView, onSwitchAccount]);

  const cancelDiscard = useCallback(() => {
    setPendingAction(null);
    setDiscardModalOpen(false);
  }, []);

  return { editPostViewDirtyRef, discardModalOpen, handleNavClick, handleAccountSwitch, confirmDiscard, cancelDiscard, resetSignal };
}
