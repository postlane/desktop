// SPDX-License-Identifier: BUSL-1.1

import { describe, expect, it } from 'vitest';
import { LLM_PROVIDERS, LLM_PROVIDER_LABELS, LLM_MODEL_CHOICES, OTHER_MODEL } from './llmProviders';

describe('LLM_PROVIDERS', () => {
  it('matches the CLI init flow\'s provider list exactly (cli/src/init/questions.ts)', () => {
    expect(LLM_PROVIDERS).toEqual([
      'anthropic', 'openai', 'google', 'mistral', 'groq',
      'deepseek', 'ollama', 'lm_studio', 'custom_openai', 'other',
    ]);
  });

  it('has no duplicate providers', () => {
    expect(new Set(LLM_PROVIDERS).size).toBe(LLM_PROVIDERS.length);
  });
});

describe('LLM_PROVIDER_LABELS', () => {
  it('has a label for every provider in LLM_PROVIDERS', () => {
    for (const provider of LLM_PROVIDERS) {
      expect(LLM_PROVIDER_LABELS[provider]).toBeTruthy();
    }
  });
});

describe('LLM_MODEL_CHOICES', () => {
  it('provides curated models only for providers the CLI curates (anthropic/openai/google)', () => {
    expect(LLM_MODEL_CHOICES['anthropic']?.length).toBeGreaterThan(0);
    expect(LLM_MODEL_CHOICES['openai']?.length).toBeGreaterThan(0);
    expect(LLM_MODEL_CHOICES['google']?.length).toBeGreaterThan(0);
  });

  it('has no curated list for providers that fall back to free-text entry', () => {
    expect(LLM_MODEL_CHOICES['mistral']).toBeUndefined();
    expect(LLM_MODEL_CHOICES['other']).toBeUndefined();
  });
});

describe('OTHER_MODEL', () => {
  it('is a distinct sentinel value, not a real model name', () => {
    expect(OTHER_MODEL).toBe('__other__');
  });
});
