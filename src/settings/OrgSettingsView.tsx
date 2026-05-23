// SPDX-License-Identifier: BUSL-1.1

import RepositoriesBlock from './RepositoriesBlock';
import SchedulerBlock from './SchedulerBlock';
import VoiceGuideBlock from './VoiceGuideBlock';
import MembersBlock from './MembersBlock';
import BillingBlock from './BillingBlock';
import ImageSearchBlock from './ImageSearchBlock';
import DirectChannelsBlock from './DirectChannelsBlock';
import type { Project } from '../types';

interface Props {
  org: Project;
}

export default function OrgSettingsView({ org }: Props) {
  const isOwner = org.is_owner;
  return (
    <div className="px-5 py-4" style={{ maxWidth: '48rem' }}>
      <p className="is-size-5 has-text-weight-semibold mb-5">{org.name} — Settings</p>
      <div style={{ display: 'flex', flexDirection: 'column', gap: '2rem' }}>
        <RepositoriesBlock projectId={org.id} projectName={org.name} isOwner={isOwner} />
        <SchedulerBlock projectId={org.id} isOwner={isOwner} />
        <DirectChannelsBlock />
        <ImageSearchBlock />
        <VoiceGuideBlock projectId={org.id} projectName={org.name} isOwner={isOwner} />
        <MembersBlock />
        <BillingBlock project={org} isOwner={isOwner} />
      </div>
    </div>
  );
}
