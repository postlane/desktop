// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { _init } from './p';

const sendBeacon = vi.fn();

beforeEach(() => {
  vi.stubGlobal('navigator', { ...globalThis.navigator, sendBeacon });
  sendBeacon.mockClear();
  sessionStorage.clear();
  document.head.innerHTML = '<script src="https://cdn.postlane.dev/p.js" data-site="test-site-token-abc"></script>';
  history.pushState({}, '', '/');
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('p.js — test_snippet_fires_only_on_postlane_utm', () => {
  it('does not call sendBeacon when utm_source is absent', () => {
    history.pushState({}, '', '/blog');
    _init();
    expect(sendBeacon).not.toHaveBeenCalled();
  });

  it('does not call sendBeacon when utm_source is not postlane', () => {
    history.pushState({}, '', '/blog?utm_source=twitter');
    _init();
    expect(sendBeacon).not.toHaveBeenCalled();
  });

  it('does not call sendBeacon when data-site attribute is missing', () => {
    document.head.innerHTML = '<script src="https://cdn.postlane.dev/p.js" defer></script>';
    history.pushState({}, '', '/blog?utm_source=postlane');
    _init();
    expect(sendBeacon).not.toHaveBeenCalled();
  });

  it('does not use data-site from a script without the cdn.postlane.dev src', () => {
    document.head.innerHTML = '<script data-site="attacker-token"></script>';
    history.pushState({}, '', '/page?utm_source=postlane');
    _init();
    expect(sendBeacon).not.toHaveBeenCalled();
  });
});

describe('p.js — payload length caps', () => {
  it('truncates path longer than 2048 chars', () => {
    const longPath = '/' + 'a'.repeat(3000);
    history.pushState({}, '', longPath + '?utm_source=postlane');
    _init();
    expect(sendBeacon).toHaveBeenCalledOnce();
    const [, body] = sendBeacon.mock.calls[0] as [string, string];
    const payload = JSON.parse(body) as Record<string, unknown>;
    expect((payload.path as string).length).toBeLessThanOrEqual(2048);
  });

  it('truncates referrer longer than 2048 chars', () => {
    history.pushState({}, '', '/page?utm_source=postlane');
    Object.defineProperty(document, 'referrer', { value: 'r'.repeat(3000), configurable: true, writable: true });
    _init();
    expect(sendBeacon).toHaveBeenCalledOnce();
    const [, body] = sendBeacon.mock.calls[0] as [string, string];
    const payload = JSON.parse(body) as Record<string, unknown>;
    expect((payload.referrer as string).length).toBeLessThanOrEqual(2048);
    Object.defineProperty(document, 'referrer', { value: '', configurable: true, writable: true });
  });
});

describe('p.js — test_snippet_sends_correct_payload', () => {
  it('calls sendBeacon with correct URL and payload fields', () => {
    history.pushState({}, '', '/blog?utm_source=postlane&utm_content=my-post&utm_medium=social&utm_campaign=q1');
    _init();
    expect(sendBeacon).toHaveBeenCalledOnce();
    const [url, body] = sendBeacon.mock.calls[0] as [string, string];
    expect(url).toBe('https://api.postlane.dev/v1/events');
    const payload = JSON.parse(body) as Record<string, unknown>;
    expect(payload.site_token).toBe('test-site-token-abc');
    expect(payload.utm_source).toBe('postlane');
    expect(payload.utm_content).toBe('my-post');
    expect(payload.utm_medium).toBe('social');
    expect(payload.utm_campaign).toBe('q1');
    expect(payload.path).toBe('/blog');
    expect(payload.session_id).toBeTruthy();
  });
});

describe('p.js — test_snippet_deduplicates_within_session', () => {
  it('reuses the same session_id across multiple calls in the same tab', () => {
    history.pushState({}, '', '/page1?utm_source=postlane');
    _init();
    history.pushState({}, '', '/page2?utm_source=postlane');
    _init();
    expect(sendBeacon).toHaveBeenCalledTimes(2);
    const [, body1] = sendBeacon.mock.calls[0] as [string, string];
    const [, body2] = sendBeacon.mock.calls[1] as [string, string];
    const p1 = JSON.parse(body1) as Record<string, unknown>;
    const p2 = JSON.parse(body2) as Record<string, unknown>;
    expect(p1.session_id).toBe(p2.session_id);
  });
});
