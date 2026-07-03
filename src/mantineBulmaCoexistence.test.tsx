// SPDX-License-Identifier: BUSL-1.1
// v2.0 checklist 24.0.2: a permanent regression check, not a one-time manual
// confirmation -- 8 more Mantine surfaces get built across this release
// after Mantine/Bulma coexistence is first established, and a one-time check
// gives no tripwire if one of them collides.
//
// This test doesn't depend on Vitest's jsdom environment actually loading
// real CSS (vitest.config.ts has no `css: true`, so stylesheet content isn't
// applied to computed styles here) -- instead it verifies the two libraries
// never render overlapping class names in the first place, which is the
// actual mechanism a collision would require. Mantine generates its own
// scoped class names (mantine-Button-root, m_xxxxxxx hashes); Bulma's are
// plain semantic names (button, is-primary). As long as neither renders a
// class name the other one also targets, there is no collision to have.

import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import { MantineProvider, Button } from '@mantine/core';
import { postlaneTheme } from './theme';

function CoexistenceHarness() {
  return (
    <MantineProvider theme={postlaneTheme}>
      <Button data-testid="mantine-button">Mantine</Button>
      <button data-testid="bulma-button" className="button is-primary">
        Bulma
      </button>
    </MantineProvider>
  );
}

describe('Mantine/Bulma coexistence', () => {
  it('renders both a Mantine and a Bulma button without throwing', () => {
    render(<CoexistenceHarness />);
    expect(screen.getByTestId('mantine-button')).toBeInTheDocument();
    expect(screen.getByTestId('bulma-button')).toBeInTheDocument();
  });

  it('does not render Bulma\'s "button" class on the Mantine button', () => {
    render(<CoexistenceHarness />);
    const mantineButton = screen.getByTestId('mantine-button');
    const classes = mantineButton.className.split(/\s+/);
    expect(classes).not.toContain('button');
    expect(classes).not.toContain('is-primary');
  });

  it('does not render any mantine-prefixed class on the Bulma button', () => {
    render(<CoexistenceHarness />);
    const bulmaButton = screen.getByTestId('bulma-button');
    const classes = bulmaButton.className.split(/\s+/);
    expect(classes.some((c) => c.toLowerCase().includes('mantine'))).toBe(false);
  });

  it('the Mantine button uses at least one Mantine-generated class', () => {
    render(<CoexistenceHarness />);
    const mantineButton = screen.getByTestId('mantine-button');
    const classes = mantineButton.className.split(/\s+/);
    expect(classes.length).toBeGreaterThan(0);
    expect(classes.some((c) => c.startsWith('m_') || c.toLowerCase().includes('mantine'))).toBe(true);
  });
});
