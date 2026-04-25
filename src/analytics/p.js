// SPDX-License-Identifier: BUSL-1.1
// Postlane attribution snippet — fires only on sessions arriving via utm_source=postlane.
// No cookies. No PII. No fingerprinting.
/* global window, document, navigator, sessionStorage, URLSearchParams, crypto, __vitest_worker__ */

const ENDPOINT = 'https://api.postlane.dev/v1/events';
const SESSION_KEY = 'postlane_sid';

function getSiteToken() {
  const el = document.querySelector('script[src*="cdn.postlane.dev"][data-site]');
  return el ? el.getAttribute('data-site') : null;
}

function getOrCreateSessionId() {
  let sid = sessionStorage.getItem(SESSION_KEY);
  if (!sid) {
    const bytes = new Uint8Array(16);
    crypto.getRandomValues(bytes);
    sid = Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
    sessionStorage.setItem(SESSION_KEY, sid);
  }
  return sid;
}

export function _init() {
  const siteToken = getSiteToken();
  if (!siteToken) return;
  const params = new URLSearchParams(window.location.search);
  if (params.get('utm_source') !== 'postlane') return;
  const payload = JSON.stringify({
    site_token: siteToken,
    utm_source: params.get('utm_source'),
    utm_medium: params.get('utm_medium'),
    utm_campaign: params.get('utm_campaign'),
    utm_content: params.get('utm_content'),
    path: window.location.pathname.slice(0, 2048),
    referrer: document.referrer.slice(0, 2048),
    session_id: getOrCreateSessionId(),
  });
  navigator.sendBeacon(ENDPOINT, payload);
}

// Auto-run when loaded as a <script> tag (CDN mode)
if (typeof window !== 'undefined' && typeof __vitest_worker__ === 'undefined') {
  _init();
}
