// SPDX-License-Identifier: BUSL-1.1
// checklist 24.4.13 — EU Article 11a withdrawal button (design brief 10,
// internal/build/design-briefs/v2.0/10-workspace-withdrawal-button.md).
//
// Three-view flow, following AccountDangerZone's DeleteModal view-state-
// machine precedent (not a native confirm() dialog):
//   Statement (Step 1, read-only consequence copy, "Continue") ->
//   Confirm (Step 2, separate view; an acknowledgment checkbox gates the
//   actual "Confirm withdrawal" button, "Back" returns to Statement) ->
//   Confirming (in flight) -> Success / Error (Error stays on the Confirm
//   view with a retry action, per the brief's "confirmation fails" state).
// No single click anywhere in this flow can complete a withdrawal.
//
// Refund/receipt copy is deliberately generic ("if you're within your
// first 14 days...") rather than previewing the exact amount or the
// account holder's email before confirming -- Article 11a's button/
// two-step/receipt requirements don't call for a pre-confirmation
// preview of Article 14(3)'s financial consequence, and the exact figure
// already appears in the real durable-medium email receipt sent
// server-side on success. Building a preview endpoint just for this
// copy was judged not worth the added surface.
//
// Built in Mantine per the design brief's resolved framework question,
// even though this component's parent (BillingBlock.tsx/OrgSettingsView)
// is still Bulma -- Postlane is moving every screen to Mantine. No icons
// -- the brand guidelines specify an iconography style but it has never
// been implemented as a real shared module anywhere in this app, and
// introducing one for a single button would be its own inconsistency.

import { useState } from 'react';
import { Button, Modal, Text, Group, Loader, Checkbox } from '@mantine/core';
import { invoke } from '../ipc/invoke';

type View = 'idle' | 'statement' | 'confirm' | 'confirming' | 'success' | 'error';

interface WithdrawResult {
  status: string;
  refunded: boolean;
  refund_amount?: number;
}

interface Props {
  projectId: string;
  workspaceName: string;
  billingActive: boolean;
}

function errorMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function StatementStep({ workspaceName, onCancel, onContinue }: { workspaceName: string; onCancel: () => void; onContinue: () => void }) {
  return (
    <>
      <Text size="sm" mb="md">
        Withdrawing from the contract for <strong>{workspaceName}</strong> will pause this
        workspace immediately. If you subscribed within the last 14 days, the unused portion of
        your current billing period will be refunded automatically to your original payment
        method. A confirmation email will be sent to your account address once withdrawal is
        confirmed.
      </Text>
      <Group justify="flex-end">
        <Button variant="subtle" onClick={onCancel}>Cancel</Button>
        <Button color="red" onClick={onContinue}>Continue</Button>
      </Group>
    </>
  );
}

function ConfirmStep({
  workspaceName, ack, onAckChange, onBack, onConfirm, error, retrying,
}: {
  workspaceName: string;
  ack: boolean;
  onAckChange: (checked: boolean) => void;
  onBack: () => void;
  onConfirm: () => void;
  error: string | null;
  retrying: boolean;
}) {
  return (
    <>
      <Text size="sm" mb="md">
        Confirm you want to withdraw from the contract for <strong>{workspaceName}</strong>. This
        takes effect right away.
      </Text>
      <Checkbox
        checked={ack}
        onChange={(e) => onAckChange(e.currentTarget.checked)}
        label={`I understand ${workspaceName} will be paused and any pro-rata refund due will be issued automatically.`}
        mb="md"
      />
      {error && <Text size="sm" c="red" mb="md">{error}</Text>}
      <Group justify="space-between">
        <Button variant="default" onClick={onBack}>Back</Button>
        <Button color="red" disabled={!ack} onClick={onConfirm}>{retrying ? 'Try again' : 'Confirm withdrawal'}</Button>
      </Group>
    </>
  );
}

function SuccessStep({ result, onDone }: { result: WithdrawResult; onDone: () => void }) {
  return (
    <>
      <Text size="sm" mb="md">
        Withdrawal confirmed — check your email for a receipt.
        {result.refunded && result.refund_amount !== undefined && (
          <> A refund of ${(result.refund_amount / 100).toFixed(2)} has been issued.</>
        )}
      </Text>
      <Group justify="flex-end">
        <Button onClick={onDone}>Done</Button>
      </Group>
    </>
  );
}

export default function WithdrawFromContractButton({ projectId, workspaceName, billingActive }: Props) {
  const [view, setView] = useState<View>('idle');
  const [ack, setAck] = useState(false);
  const [result, setResult] = useState<WithdrawResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  if (!billingActive) {
    return (
      <Button size="xs" variant="default" disabled title="This workspace isn't on an active plan">
        Withdraw from contract
      </Button>
    );
  }

  async function handleConfirm() {
    setView('confirming');
    setError(null);
    try {
      const response = await invoke<WithdrawResult>('withdraw_from_contract', { projectId });
      setResult(response);
      setView('success');
    } catch (e) {
      setError(errorMessage(e));
      setView('error');
    }
  }

  function handleClose() {
    setView('idle');
    setAck(false);
    setResult(null);
    setError(null);
  }

  return (
    <>
      <Button size="xs" variant="default" onClick={() => setView('statement')}>
        Withdraw from contract
      </Button>
      <Modal opened={view !== 'idle'} onClose={handleClose} title="Withdraw from contract" closeOnClickOutside={view !== 'confirming'}>
        {view === 'statement' && (
          <StatementStep workspaceName={workspaceName} onCancel={handleClose} onContinue={() => setView('confirm')} />
        )}
        {(view === 'confirm' || view === 'error') && (
          <ConfirmStep
            workspaceName={workspaceName}
            ack={ack}
            onAckChange={setAck}
            onBack={() => setView('statement')}
            onConfirm={handleConfirm}
            error={view === 'error' ? error : null}
            retrying={view === 'error'}
          />
        )}
        {view === 'confirming' && <Group justify="center" py="md"><Loader size="sm" /></Group>}
        {view === 'success' && result && <SuccessStep result={result} onDone={handleClose} />}
      </Modal>
    </>
  );
}
