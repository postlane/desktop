// SPDX-License-Identifier: BUSL-1.1
// v2.0 checklist 24.0: jsdom has no window.matchMedia implementation, but
// Mantine's MantineProvider calls it on mount (color-scheme detection) --
// every test that renders anything wrapped in MantineProvider needs this,
// not just this release's own coexistence check, so it's a global setup
// polyfill rather than a per-test-file mock.
Object.defineProperty(window, 'matchMedia', {
  writable: true,
  configurable: true,
  value: (query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  }),
});

// jsdom has no ResizeObserver either -- needed by Mantine components that
// measure their own size (Select/Combobox's internal ScrollArea, checklist
// 24.4.15a). Global polyfill for the same reason as matchMedia above.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
Object.defineProperty(window, 'ResizeObserver', {
  writable: true,
  configurable: true,
  value: ResizeObserverStub,
});
