// SPDX-License-Identifier: BUSL-1.1
// checklist 24.3.4 -- Step 5: single attribution toggle, on by default.

import { useState } from 'react';
import { Button, Group, Stack, Switch } from '@mantine/core';

export interface AttributionPatch {
  attribution: boolean;
}

interface Props {
  onNext: (patch: AttributionPatch) => void;
  onBack: () => void;
}

export default function StepAttribution({ onNext, onBack }: Props) {
  const [attribution, setAttribution] = useState(true);

  return (
    <Stack gap="sm">
      <Switch
        label="Append &quot;Built with Postlane&quot; to posts?"
        checked={attribution}
        onChange={(e) => setAttribution(e.currentTarget.checked)}
      />
      <Group justify="space-between">
        <Button variant="subtle" onClick={onBack}>Back</Button>
        <Button onClick={() => onNext({ attribution })}>Next</Button>
      </Group>
    </Stack>
  );
}
