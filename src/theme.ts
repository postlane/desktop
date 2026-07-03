// SPDX-License-Identifier: BUSL-1.1
//
// v2.0 checklist 24.0.3: postlaneTheme is derived from postlane/web's
// already-established Mantine theme (src/app/providers.tsx there, behind the
// marketing-site redesign shipped before v1.4) -- NOT from desktop's Bulma
// variables ($primary, $grey-dark, $link). That's the corrected direction
// per the brief's "UI framework migration" note: this release aligns
// desktop's brand to the web app's current one, not the reverse.
//
// postlane/web and postlane/desktop are separate repos with no shared
// package between them, so this is a manual port, not an import -- web's
// theme is the source of truth. When web's providers.tsx changes, port the
// same values here by hand; there is no automated sync.

import { createTheme, type MantineColorsTuple } from '@mantine/core';

const cobalt: MantineColorsTuple = [
  '#eef2ff', '#dbe3ff', '#b9c8ff', '#8ea6ff', '#5d7cff',
  '#2e5bff', '#1f46e0', '#1736b4', '#162e8c', '#0f2068',
];

const gray: MantineColorsTuple = [
  '#f6f7f9', '#eef0f3', '#e6e8ec', '#dadde3', '#c3c8d1',
  '#9097a1', '#6b727e', '#4d5460', '#363c46', '#23272f',
];

const dark: MantineColorsTuple = [
  '#eceef1', '#aeb4be', '#7d838e', '#4d5460', '#262b33',
  '#1c2026', '#15181d', '#0b0d10', '#080a0c', '#050607',
];

const signal: MantineColorsTuple = [
  '#e6f9ef', '#c3f0d9', '#94e4bb', '#5fd69b', '#33c97f',
  '#1fc16b', '#17a059', '#127f47', '#0e6035', '#094426',
];

const amber: MantineColorsTuple = [
  '#fef6e7', '#fde9c2', '#fbd690', '#f9c05a', '#f7ab30',
  '#f59e0b', '#cc8109', '#a26607', '#7d4f08', '#5f3c0a',
];

const rose: MantineColorsTuple = [
  '#fde8ee', '#fbcdd9', '#f79db4', '#f4708f', '#f24c74',
  '#f0436f', '#cf2f59', '#a82547', '#831d39', '#62182d',
];

// Font stack adapted, not ported verbatim: web's fontFamily values reference
// CSS custom properties set up by next/font (--font-hanken, --font-jetbrains,
// --font-schibsted), which don't exist in this Vite+React app. Substituted
// with 'Inter' -- the font already loaded by index.css's existing
// `font-family: Inter, system-ui, sans-serif` -- rather than pulling in a new
// font just to match web's headings typeface. Colors/spacing/radius/shadows
// (the actual brand identity) are ported as-is.
export const postlaneTheme = createTheme({
  primaryColor: 'cobalt',
  primaryShade: { light: 5, dark: 4 },
  colors: { cobalt, gray, dark, signal, amber, rose },
  white: '#ffffff',
  black: '#15181d',
  fontFamily: 'Inter, system-ui, sans-serif',
  fontFamilyMonospace: 'ui-monospace, monospace',
  headings: {
    fontFamily: 'Inter, system-ui, sans-serif',
    fontWeight: '700',
    sizes: {
      h1: { fontSize: '2.75rem', lineHeight: '1.05', fontWeight: '800' },
      h2: { fontSize: '2rem', lineHeight: '1.1', fontWeight: '700' },
      h3: { fontSize: '1.5rem', lineHeight: '1.15', fontWeight: '700' },
      h4: { fontSize: '1.175rem', lineHeight: '1.25', fontWeight: '600' },
    },
  },
  defaultRadius: 'md',
  radius: { xs: '6px', sm: '8px', md: '10px', lg: '14px', xl: '20px' },
  shadows: {
    xs: '0 1px 2px rgba(15,17,21,.06), 0 1px 1px rgba(15,17,21,.04)',
    sm: '0 1px 2px rgba(15,17,21,.06), 0 1px 1px rgba(15,17,21,.04)',
    md: '0 4px 12px rgba(15,17,21,.07), 0 2px 4px rgba(15,17,21,.05)',
    lg: '0 18px 40px -12px rgba(15,17,21,.18), 0 6px 14px rgba(15,17,21,.08)',
    xl: '0 24px 56px -16px rgba(15,17,21,.22), 0 8px 18px rgba(15,17,21,.10)',
  },
  cursorType: 'pointer',
  components: {
    Button: {
      defaultProps: { radius: 'md' },
      styles: { root: { fontWeight: 600, letterSpacing: '-0.01em' } },
    },
    Card: {
      defaultProps: { radius: 'lg', withBorder: true },
    },
    Badge: {
      styles: {
        root: { fontWeight: 600, letterSpacing: '0.02em' },
      },
    },
    TextInput: { defaultProps: { radius: 'md' } },
    Textarea: { defaultProps: { radius: 'md' } },
  },
});
