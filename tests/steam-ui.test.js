const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..');

test('Idle Steam mode structure has Library and Active sessions modes', () => {
  const html = fs.readFileSync(path.join(root, 'public/index.html'), 'utf8');
  assert.ok(html.includes('data-steam-sub-mode="library"'), 'Missing library sub-mode');
  assert.ok(html.includes('data-steam-sub-mode="active"'), 'Missing active sessions sub-mode');
  assert.ok(html.includes('id="steam-library-mode-view"'), 'Missing library container');
  assert.ok(html.includes('id="steam-active-mode-view"'), 'Missing active sessions container');
  assert.ok(html.includes('id="steam-active-nav-badge"'), 'Missing active sessions nav badge');
});

test('Active sessions panel is exclusively inside steam-active-mode-view', () => {
  const html = fs.readFileSync(path.join(root, 'public/index.html'), 'utf8');
  const libStart = html.indexOf('id="steam-library-mode-view"');
  const activeStart = html.indexOf('id="steam-active-mode-view"');
  const sessionPanel = html.indexOf('id="steam-sessions-panel"');
  assert.ok(libStart !== -1, 'Library view must exist');
  assert.ok(activeStart !== -1, 'Active sessions view must exist');
  assert.ok(sessionPanel !== -1, 'Sessions panel must exist');
  assert.ok(sessionPanel > activeStart, 'Sessions panel must be inside active sessions view, not library view');
  assert.ok(html.includes('id="steam-empty-active"'), 'Empty active state must exist');
  assert.equal(html.includes('id="steam-browse-library-btn"'), false, 'Active sessions must not duplicate the library navigation');
  assert.ok(html.includes('id="steam-pause-all"'), 'Pause all button must exist');
  assert.ok(html.includes('id="steam-stop-all"'), 'Stop all button must exist');
});

test('Trading cards UI elements, dialogs, and inputs are completely removed', () => {
  const html = fs.readFileSync(path.join(root, 'public/index.html'), 'utf8');
  assert.equal(html.includes('data-steam-sub-mode="cards"'), false, 'Cards sub-mode must not exist');
  assert.equal(html.includes('id="steam-cards-mode-view"'), false, 'Cards container must not exist');
  assert.equal(html.includes('cards-settings-dialog'), false, 'Cards dialog must not exist');
  assert.equal(html.includes('cards-session-id'), false, 'Cards credentials inputs must not exist');
  assert.equal(html.includes('cards-login-secure'), false, 'Cards credentials inputs must not exist');
  assert.equal(html.includes('cards-parental'), false, 'Cards credentials inputs must not exist');
  assert.equal(html.includes('js/card-drops.js'), false, 'card-drops.js script must not be referenced');
  assert.equal(html.includes('js/cards-ui.js'), false, 'cards-ui.js script must not be referenced');
  assert.equal(html.includes('js/steam-badges-collector.js'), false, 'steam-badges-collector.js script must not be referenced');
});

test('No glowing guidance effects or pulsing outlines exist in styling', () => {
  const css = fs.readFileSync(path.join(root, 'public/css/lootlazy.css'), 'utf8');
  assert.equal(/@keyframes\s+pulse/i.test(css), false, 'No pulsing keyframes allowed');
  assert.equal(/box-shadow:[^;]*0\s+0\s+(?:10|15|20|25)px/i.test(css), false, 'No glowing box-shadow allowed');
});
