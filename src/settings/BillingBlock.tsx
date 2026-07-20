// SPDX-License-Identifier: BUSL-1.1

import { openUrl } from '@tauri-apps/plugin-opener';
import { useProjectsContext } from '../context/ProjectsProvider';
import WithdrawFromContractButton from './WithdrawFromContractButton';
import type { Project } from '../types';

interface Props {
  project: Project;
  isOwner: boolean;
}

export default function BillingBlock({ project, isOwner }: Props) {
  const { refresh } = useProjectsContext();

  return (
    <div>
      <p className="is-size-6 has-text-weight-medium mb-3">Billing</p>
      <p className="is-size-7 mb-2">
        Plan: <strong className="is-capitalized">{project.tier}</strong>
      </p>
      {!project.billing_active && (
        <div role="alert" className="notification is-danger is-light py-2 px-3 mb-3">
          <p className="is-size-7">Your billing is inactive. Approve is disabled until resolved.</p>
          <button className="button is-small is-danger is-light mt-2" onClick={refresh}>
            I've updated my billing — Refresh
          </button>
        </div>
      )}
      {isOwner && (
        <div style={{ display: 'flex', gap: '0.5rem' }}>
          <button className="button is-small is-light" onClick={() => openUrl('https://postlane.dev/billing')}>
            Manage billing
          </button>
          <WithdrawFromContractButton
            projectId={project.id}
            workspaceName={project.name}
            billingActive={project.billing_active}
          />
        </div>
      )}
    </div>
  );
}
