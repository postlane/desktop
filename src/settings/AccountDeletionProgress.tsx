// SPDX-License-Identifier: BUSL-1.1
// §22.7.6/22.7.7 — Account deletion step machine.

import { useState, useEffect, useRef } from 'react';
import { invoke } from '../ipc/invoke';
import { ErrorCode } from './ErrorCode';

// ── Types ─────────────────────────────────────────────────────────────────────

interface Props { deleteWorkspaceDirs: boolean; onDeleted: () => void; onAbort: () => void; }

interface PhaseResult { phase: number; message: string; next_phase: number | null; }
interface PhaseError { phase: number; code: string; message: string; skippable: boolean; }

type StepState =
  | { kind: 'running'; message: string }
  | { kind: 'error'; error: PhaseError; phase: number }
  | { kind: 'done' };

// ── Error panel sub-components ────────────────────────────────────────────────

interface ErrorPanelProps {
  error: PhaseError;
  onRetry: () => void;
  onSkip: () => void;
  onAbort: () => void;
}

function ErrorPanel({ error, onRetry, onSkip, onAbort }: ErrorPanelProps) {
  const isCritical = !error.skippable;
  // Phase 0 is the pre-flight session check. Its failure message (sign-in guidance) is
  // self-explanatory — do not append the Step-5-specific "cannot be skipped" explanation.
  const isPreflightFailure = error.phase === 0;
  return (
    <div>
      <ErrorCode code={error.code} message={error.message} />
      {isCritical ? (
        <>
          {!isPreflightFailure && (
            <p className="is-size-7 mb-2">
              This step cannot be skipped. Skipping would leave your account record on
              Postlane&apos;s servers in an unrecoverable state.
            </p>
          )}
          <div className="is-flex" style={{ gap: '0.5rem' }}>
            <button className="button is-small is-danger" onClick={onRetry}>Retry</button>
            <button className="button is-small" onClick={onAbort}>Abort</button>
          </div>
        </>
      ) : (
        <>
          <div className="is-flex" style={{ gap: '0.5rem' }}>
            <button className="button is-small is-danger" onClick={onRetry}>Retry</button>
            <button className="button is-small" onClick={onSkip}>Skip and continue</button>
          </div>
          <p className="is-size-7 has-text-grey mt-1">
            Skipping this step may leave orphaned data on Postlane&apos;s servers.
          </p>
        </>
      )}
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

export default function AccountDeletionProgress({ deleteWorkspaceDirs, onDeleted, onAbort }: Props) {
  const [stepState, setStepState] = useState<StepState>({ kind: 'running', message: 'Verifying session…' });
  const runningPhase = useRef<number>(0);

  async function runPhase(phase: number) {
    setStepState({ kind: 'running', message: getDefaultMessage(phase) });
    try {
      const result = await invoke<PhaseResult>('run_deletion_phase', { phase, deleteWorkspaceDirs });
      if (result.next_phase !== null && result.next_phase !== undefined) {
        runningPhase.current = result.next_phase;
        runPhase(result.next_phase);
      } else {
        setStepState({ kind: 'done' });
        invoke('sign_out').catch(() => {}).finally(() => onDeleted());
      }
    } catch (e) {
      const err = e as PhaseError;
      setStepState({ kind: 'error', error: err, phase });
    }
  }

  function getDefaultMessage(phase: number): string {
    const msgs: Record<number, string> = {
      0: 'Verifying session…',
      1: 'Removing project data…',
      2: 'Removing project data…',
      3: 'Revoking integrations…',
      4: 'Clearing credentials…',
      5: 'Removing account record…',
      6: 'Cleaning up local files…',
      7: 'Removing workspace files…',
    };
    return msgs[phase] ?? 'Finishing…';
  }

  useEffect(() => { runPhase(0); }, []); // eslint-disable-line react-hooks/exhaustive-deps

  if (stepState.kind === 'done') {
    return (
      <p className="has-text-success">
        Your account has been deleted. All credentials and server data have been removed.
      </p>
    );
  }

  if (stepState.kind === 'error') {
    return (
      <ErrorPanel
        error={stepState.error}
        onRetry={() => runPhase(stepState.phase)}
        onSkip={() => runPhase(stepState.phase + 1)}
        onAbort={onAbort}
      />
    );
  }

  return <p className="has-text-grey">{stepState.message}</p>;
}
