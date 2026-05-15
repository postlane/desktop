// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, type ChangeEvent } from 'react';
import { invoke } from '../ipc/invoke';
import WizardShell from './WizardShell';

const DEFAULT_TONE = 'Direct and professional. Technically precise. No marketing language.';
const DEFAULT_AUDIENCE = 'developers and technical users.';

// Keep in sync with FORBIDDEN_PHRASES in prompts/runner/validation.ts
const AVOID_DEFAULT = [
  '"Excited to share" / "Thrilled to announce"',
  '"Game-changing" / "Revolutionary" / "Groundbreaking"',
  '"Dive into" / "Delve into"',
  '"Leverage" as a verb',
  '"Seamlessly"',
  '"The future of [category]"',
  'Any sentence starting with "I\'m proud to" or "I\'m humbled to"',
].join('\n');

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
  lines.push('## Audience', audience, '', '## Tone', tone, '');
  if (fields.avoid.trim()) {
    lines.push('## Never use');
    fields.avoid.split('\n').map((l) => l.trim()).filter(Boolean).forEach((l) => lines.push(`- ${l}`));
    lines.push('');
  }
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
      {field('vpc-avoid', 'Avoid', 'avoid', 'e.g. Em dashes, corporate buzzwords, passive voice', 7)}
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
  const [fields, setFields] = useState<VoiceGuideFields>({ description: '', audience: '', tone: '', avoid: AVOID_DEFAULT, examples: '' });
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

  const handleChange: OnChange = (key, value) => setFields((prev) => ({ ...prev, [key]: value }));

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
      <VoiceGuideForm fields={fields} onChange={handleChange} saveError={saveError} />
    </WizardShell>
  );
}
