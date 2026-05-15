// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import WizardShell from './WizardShell';
import { VoiceGuideForm, VoiceGuideFields, EMPTY_FIELDS, buildVoiceGuide } from '../settings/VoiceGuideForm';

export { buildVoiceGuide };
export type { VoiceGuideFields };

interface Props {
  workspaceId: string;
  workspaceName: string;
  onNext: () => void;
  onBack: () => void;
}

export default function ModalProjectContext({ workspaceId, workspaceName, onNext, onBack }: Props) {
  const [fields, setFields] = useState<VoiceGuideFields>(EMPTY_FIELDS);
  const [saveError, setSaveError] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    invoke('get_voice_guide_fields', { projectId: workspaceId })
      .then((data) => {
        if (data !== null && typeof data === 'object' && !Array.isArray(data)) {
          const incoming = data as Partial<VoiceGuideFields>;
          setFields((prev) => ({
            description: incoming.description ?? prev.description,
            audience: incoming.audience ?? prev.audience,
            tone: incoming.tone ?? prev.tone,
            avoid: incoming.avoid ?? prev.avoid,
            examples: incoming.examples ?? prev.examples,
          }));
        }
      })
      .catch(() => undefined);
  }, [workspaceId]);

  const handleChange = (key: keyof VoiceGuideFields, value: string) => setFields((prev) => ({ ...prev, [key]: value }));

  async function handleNext() {
    setSaving(true);
    setSaveError(false);
    try {
      await invoke('save_project_voice_guide', {
        projectId: workspaceId,
        voiceGuide: buildVoiceGuide(fields, workspaceName),
        voiceGuideFields: fields,
      });
    } catch {
      setSaveError(true);
    } finally {
      setSaving(false);
      onNext();
    }
  }

  return (
    <WizardShell step={6} totalSteps={7} title="Your voice"
      subtitle={`Help Postlane write posts that sound like you. This voice guide is applied to every post drafted for ${workspaceName || 'this project'}. All fields are optional — you can edit it anytime in Project Settings.`}
      onNext={handleNext} nextLabel={saving ? 'Saving…' : 'Next'} nextDisabled={saving} onBack={onBack}>
      <VoiceGuideForm fields={fields} onChange={handleChange} onApplyTemplate={setFields} saveError={saveError} />
    </WizardShell>
  );
}
