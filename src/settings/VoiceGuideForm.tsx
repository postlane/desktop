// SPDX-License-Identifier: BUSL-1.1

import type { ChangeEvent } from 'react';

// ── Types and constants ───────────────────────────────────────────────────────

const DEFAULT_TONE = 'Direct and professional. Technically precise. No marketing language.';
const DEFAULT_AUDIENCE = 'developers and technical users.';

// Keep in sync with FORBIDDEN_PHRASES in prompts/runner/validation.ts
export const AVOID_DEFAULT = [
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

export const EMPTY_FIELDS: VoiceGuideFields = {
  description: '', audience: '', tone: '', avoid: AVOID_DEFAULT, examples: '',
};

export const VOICE_GUIDE_TEMPLATES: { label: string; fields: VoiceGuideFields }[] = [
  {
    label: 'Professional',
    fields: {
      description: '',
      audience: 'Industry peers and decision-makers who value directness',
      tone: 'Direct and confident. Lead with the key point. Active voice. No hedging or filler.',
      avoid: AVOID_DEFAULT,
      examples: '',
    },
  },
  {
    label: 'Conversational',
    fields: {
      description: '',
      audience: 'Curious generalists who value plain language',
      tone: 'Warm and easy to read. Short sentences. Sound like a thoughtful colleague, not a press release.',
      avoid: AVOID_DEFAULT,
      examples: '',
    },
  },
  {
    label: 'Technical',
    fields: {
      description: '',
      audience: 'Developers and technical practitioners',
      tone: 'Precise and specific. Accurate technical vocabulary. Concrete details — versions, metrics, commands.',
      avoid: AVOID_DEFAULT,
      examples: '',
    },
  },
];

// ── Build function ────────────────────────────────────────────────────────────

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

// ── Form component ────────────────────────────────────────────────────────────

function FieldRow({ id, label, placeholder, value, onChange, rows }: {
  id: string; label: string; placeholder: string; value: string;
  onChange: (_v: string) => void; rows?: number;
}) {
  const shared = { id, className: `${rows ? 'textarea' : 'input'} is-small`, placeholder, value,
    onChange: (e: ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => onChange(e.target.value) };
  return (
    <div className="mb-4">
      <label className="label is-small" htmlFor={id}>{label}</label>
      {rows ? <textarea {...shared} rows={rows} /> : <input {...shared} type="text" />}
    </div>
  );
}

interface FormProps {
  fields: VoiceGuideFields;
  onChange: (_key: keyof VoiceGuideFields, _value: string) => void;
  onApplyTemplate: (_fields: VoiceGuideFields) => void;
  saveError?: boolean;
}

export function VoiceGuideForm({ fields, onChange, onApplyTemplate, saveError }: FormProps) {
  return (
    <>
      <div className="is-flex is-align-items-center mb-4" style={{ gap: '0.5rem', flexWrap: 'wrap' }}>
        <span className="is-size-7 has-text-grey">Start from a template:</span>
        {VOICE_GUIDE_TEMPLATES.map((t) => (
          <button key={t.label} className="button is-small is-light" onClick={() => onApplyTemplate(t.fields)}>
            {t.label}
          </button>
        ))}
      </div>
      {saveError && (
        <p className="notification is-warning is-light py-2 px-3 mb-4 is-size-7">
          Could not save voice guide — your settings were not lost. You can update this in Project Settings.
        </p>
      )}
      <FieldRow id="vgf-identity" label="Identity" placeholder="e.g. Hugo, indie developer building dev tools in public"
        value={fields.description} onChange={(v) => onChange('description', v)} />
      <FieldRow id="vgf-audience" label="Audience" placeholder="e.g. Developers who ship their own products"
        value={fields.audience} onChange={(v) => onChange('audience', v)} />
      <FieldRow id="vgf-tone" label="Tone" placeholder="e.g. Direct and technical. Short sentences. No hedging."
        value={fields.tone} onChange={(v) => onChange('tone', v)} rows={2} />
      <FieldRow id="vgf-avoid" label="Avoid" placeholder="e.g. Em dashes, corporate buzzwords, passive voice"
        value={fields.avoid} onChange={(v) => onChange('avoid', v)} rows={7} />
      <FieldRow id="vgf-examples" label="Example posts" placeholder="Paste 1–3 posts you've already written. Nothing beats showing the LLM real examples."
        value={fields.examples} onChange={(v) => onChange('examples', v)} rows={4} />
    </>
  );
}
