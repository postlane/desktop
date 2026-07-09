// SPDX-License-Identifier: BUSL-1.1

// Mirrors cli/src/init/questions.ts's provider/model lists exactly -- values
// must stay byte-identical to that file to avoid config.json drift between
// CLI-init'd and wizard-init'd repos. Cross-package import isn't possible
// (separate TS packages), so this is a deliberate, documented duplication;
// keep in sync with the CLI when either changes.

export const LLM_PROVIDERS: readonly string[] = [
  'anthropic', 'openai', 'google', 'mistral', 'groq',
  'deepseek', 'ollama', 'lm_studio', 'custom_openai', 'other',
];

export const LLM_PROVIDER_LABELS: Record<string, string> = {
  anthropic: 'Anthropic',
  openai: 'OpenAI',
  google: 'Google',
  mistral: 'Mistral',
  groq: 'Groq',
  deepseek: 'DeepSeek',
  ollama: 'Ollama',
  lm_studio: 'LM Studio',
  custom_openai: 'Custom OpenAI',
  other: 'Other',
};

/** Curated model lists — update alongside cli/src/init/questions.ts's MODEL_CHOICES.
 *  A provider absent here falls back to a free-text model input in Step 3. */
export const LLM_MODEL_CHOICES: Record<string, string[]> = {
  anthropic: ['claude-sonnet-4-6', 'claude-opus-4-7', 'claude-haiku-4-5-20251001'],
  openai: ['gpt-4o', 'gpt-4o-mini', 'o4-mini', 'o3'],
  google: ['gemini-2.5-pro', 'gemini-2.0-flash'],
};

/** Sentinel value for "Other (enter manually)" in the model picker — never a real model name. */
export const OTHER_MODEL = '__other__';
