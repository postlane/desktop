// SPDX-License-Identifier: BUSL-1.1

import { useState, type ChangeEvent } from 'react';
import { invoke } from '../ipc/invoke';
import WizardShell from './WizardShell';

const DEFAULT_TONE = 'Direct and professional. Technically precise. No marketing language.';
const DEFAULT_AUDIENCE = 'developers and technical users.';

const STANDARD_SEVEN = [
  '"excited to share" / "thrilled to announce"',
  '"game-changing" / "revolutionary" / "groundbreaking"',
  '"dive into" / "delve into"',
  '"leverage" as a verb',
  '"seamlessly"',
  '"the future of [category]"',
  'any sentence starting with "I\'m proud to"',
];

export interface VoiceGuideFields {
  description: string;
  audience: string;
  tone: string;
  avoid: string;
  examples: string;
}

export function buildVoiceGuide(fields: VoiceGuideFields, workspaceName: string): string {
  const tone = fields.tone.trim() || DEFAULT_TONE;
  const audience = fields.audience.trim() || DEFAULT_AUDIENCE;
  const lines: string[] = [`# Voice guide — ${workspaceName}`, ''];
  if (fields.description.trim()) lines.push('## Identity', fields.description.trim(), '');
  lines.push('## Audience', audience, '', '## Tone', tone, '', '## Never use');
  STANDARD_SEVEN.forEach((p) => lines.push(`- ${p}`));
  if (fields.avoid.trim()) {
    fields.avoid.split('\n').map((l) => l.trim()).filter(Boolean).forEach((l) => lines.push(`- ${l}`));
  }
  lines.push('');
  if (fields.examples.trim()) lines.push('## Example posts', fields.examples.trim(), '');
  return lines.join('\n');
}

type OnChange = (field: keyof VoiceGuideFields, value: string) => void;

interface FormProps {
  fields: VoiceGuideFields;
  onChange: OnChange;
  saveError: boolean;
}

function VoiceGuideForm({ fields, onChange, saveError }: FormProps) {
  const field = (id: string, label: string, key: keyof VoiceGuideFields, placeholder: string, rows?: number) => {
    const shared = { id, className: `${rows ? 'textarea' : 'input'} is-small`, placeholder, value: fields[key],
      onChange: (e: ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => onChange(key, e.target.value) };
    return (
      <div className="mb-4">
        <label className="label is-small" htmlFor={id}>{label}</label>
        {rows ? <textarea {...shared} rows={rows} /> : <input {...shared} type="text" />}
      </div>
    );
  };
  return (
    <>
      {saveError && (
        <p className="notification is-warning is-light py-2 px-3 mb-4 is-size-7">
          Could not save voice guide — your settings were not lost. You can update this in Project Settings.
        </p>
      )}
      {field('vpc-identity', 'Identity', 'description', 'e.g. Hugo, indie developer building dev tools in public')}
      {field('vpc-audience', 'Audience', 'audience', 'e.g. Developers who ship their own products')}
      {field('vpc-tone', 'Tone', 'tone', 'e.g. Direct and technical. Short sentences. No hedging.', 2)}
      {field('vpc-avoid', 'Avoid', 'avoid', 'e.g. Em dashes, corporate buzzwords, passive voice', 2)}
      {field('vpc-examples', 'Example posts', 'examples', 'Paste 1–3 posts you\'ve already written. Nothing beats showing the LLM real examples.', 4)}
    </>
  );
}

interface Props {
  workspaceId: string;
  workspaceName: string;
  onNext: () => void;
  onBack: () => void;
}

export default function ModalProjectContext({ workspaceId, workspaceName, onNext, onBack }: Props) {
  const [fields, setFields] = useState<VoiceGuideFields>({ description: '', audience: '', tone: '', avoid: '', examples: '' });
  const [saveError, setSaveError] = useState(false);
  const [saving, setSaving] = useState(false);

  const handleChange: OnChange = (key, value) => setFields((prev) => ({ ...prev, [key]: value }));

  async function handleNext() {
    setSaving(true);
    setSaveError(false);
    try {
      await invoke('save_project_voice_guide', { projectId: workspaceId, voiceGuide: buildVoiceGuide(fields, workspaceName) });
    } catch {
      setSaveError(true);
    } finally {
      setSaving(false);
      onNext();
    }
  }

  return (
    <WizardShell step={6} totalSteps={7} title="Your voice"
      subtitle="Help Postlane write posts that sound like you. Skip any field to use professional defaults."
      onNext={handleNext} nextLabel={saving ? 'Saving…' : 'Next'} nextDisabled={saving} onBack={onBack}>
      <VoiceGuideForm fields={fields} onChange={handleChange} saveError={saveError} />
    </WizardShell>
  );
}
