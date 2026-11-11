// Run without package changes:
// npm --prefix /tmp/rak-ui-tools install --no-audit --no-fund jsdom
// NODE_PATH=/tmp/rak-ui-tools/node_modules node --test tests/ui.test.cjs
// These DOM tests exercise the UI-focus shortcuts, not native Wry/content focus.
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { JSDOM, VirtualConsole } = require('jsdom');
const html = fs.readFileSync(path.join(__dirname, '../src/ui.html'), 'utf8');
const homeHtml = fs.readFileSync(path.join(__dirname, '../src/home.html'), 'utf8');
const mainRs = fs.readFileSync(path.join(__dirname, '../src/main.rs'), 'utf8');

function fixture(ipc = true) {
  const messages = [], errors = [];
  const console = new VirtualConsole();
  console.on('jsdomError', error => errors.push(error));
  const dom = new JSDOM(html, {
    runScripts: 'dangerously', pretendToBeVisual: true, virtualConsole: console,
    beforeParse(window) { if (ipc) window.ipc = { postMessage: message => messages.push(message) }; }
  });
  const w = dom.window, d = w.document;
  const api = {
    w, d, dom, messages, errors,
    set(tabs, active = tabs[0]?.id ?? 0, islands = []) { w.rak.setTabs(tabs, active, islands); },
    tab(id) { return d.querySelector(`.tab[data-tab="${id}"]`); },
    handle(id) { return d.querySelector(`.island-handle[data-island="${id}"]`); },
    click(node, options = {}) { assert.ok(node, 'click target exists'); node.dispatchEvent(new w.MouseEvent('click', { bubbles: true, ...options })); },
    right(node) { assert.ok(node, 'context target exists'); node.dispatchEvent(new w.MouseEvent('contextmenu', { bubbles: true, cancelable: true })); },
    key(key, options = {}, node = d) { const e = new w.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...options }); node.dispatchEvent(e); return e; },
    drag(source, target, expectedAllowed = true) {
      const transfer = { effectAllowed: '', dropEffect: '', setData() {} };
      const start = new w.Event('dragstart', { bubbles: true, cancelable: true }); Object.defineProperty(start, 'dataTransfer', { value: transfer }); source.dispatchEvent(start);
      const over = new w.Event('dragover', { bubbles: true, cancelable: true }); Object.defineProperty(over, 'dataTransfer', { value: transfer }); target.dispatchEvent(over);
      assert.equal(over.defaultPrevented, expectedAllowed, 'drop eligibility');
      target.dispatchEvent(new w.Event('drop', { bubbles: true, cancelable: true }));
      source.dispatchEvent(new w.Event('dragend', { bubbles: true }));
    },
    last() { return messages.at(-1); },
    done() { assert.deepEqual(errors, [], 'no script exceptions'); dom.window.close(); }
  };
  assert.deepEqual(errors, []);
  return api;
}
const tab = (id, island = null, privateMode = false, title = `Tab ${id}`) => ({ id, title, url: `https://example.com/${id}`, private: privateMode, island });
const group = (id, collapsed = false, privateMode = false, name = `Insel ${id}`, color = '#19d3ff') => ({ id, name, color, collapsed, private: privateMode });

test('homepage search focus does not paint a mismatched input fill', () => {
  assert.match(homeHtml, /\.search input:focus\{background:transparent\}/);
  assert.match(homeHtml, /\.search:focus-within\{border-color:var\(--accent\)\}/);
});

test('fullscreen relayout is repeated after native window size changes', () => {
  assert.match(mainRs, /fn toggle_fullscreen\(&self, window: &Window\)/);
  assert.match(mainRs, /WindowEvent::ScaleFactorChanged \{ \.\. \}/);
  assert.match(mainRs, /UserEvent::Relayout/);
});

// Browser layout is checked separately; jsdom cannot measure actual scroll widths.
test('fixed-height layout, fixed controls, safe tab titles, and existing IPC controls', () => {
  const f = fixture();
  assert.equal(f.last(), 'ready');
  f.set(Array.from({ length: 12 }, (_, i) => tab(i + 1, null, false, i ? `Tab ${i + 1}` : '<img src=x onerror=alert(1)>')));
  assert.equal(f.d.querySelectorAll('.tab').length, 12);
  assert.equal(f.tab(1).querySelector('.title').textContent, '<img src=x onerror=alert(1)>');
  assert.equal(f.d.querySelector('img'), null);
  assert.equal(f.d.getElementById('tab-list').contains(f.d.getElementById('newtab')), false);
  for (const [id, message] of [['newtab', 'new'], ['privtab', 'newp'], ['back', 'back'], ['settings', 'settings']]) {
    f.click(f.d.getElementById(id)); assert.equal(f.last(), message);
    assert.ok(f.d.getElementById(id).getAttribute('aria-label'));
  }
  const styles = f.d.querySelector('style').textContent;
  assert.match(styles, /body\s*\{[^}]*height:116px/);
  assert.match(styles, /#tabs\s*\{[^}]*height:40px/);
  assert.match(styles, /#bar\s*\{[^}]*height:44px/);
  assert.match(styles, /#bm\s*\{[^}]*height:32px/);
  assert.match(styles, /#tab-scroll\s*\{[^}]*overflow-x:auto/);
  f.click(f.tab(2)); assert.equal(f.last(), 'switch:2');
  f.click(f.tab(2).querySelector('.x')); assert.equal(f.last(), 'close:2');
  f.done();
});

test('expanded wrapper and collapsed active handle retain the active page state', () => {
  const f = fixture(); const tabs = [tab(1, 8), tab(2, 8), tab(3)];
  f.set(tabs, 2, [group(8)]);
  assert.equal(f.d.querySelector('[data-island="8"].island').style.getPropertyValue('--island-color'), '#19d3ff');
  assert.equal(f.handle(8).getAttribute('aria-expanded'), 'true');
  f.click(f.handle(8)); assert.equal(f.last(), 'island-toggle:8');
  f.set(tabs, 2, [group(8, true)]);
  assert.equal(f.tab(2), null);
  assert.ok(f.handle(8).classList.contains('contains-active'));
  assert.match(f.handle(8).getAttribute('aria-label'), /2 Tabs.*aktiven Tab/);
  assert.equal(f.d.getElementById('url').value, 'https://example.com/2');
  assert.equal(f.handle(8).getAttribute('aria-expanded'), 'false');
  assert.equal(f.d.querySelector('#island-members-8').hidden, true);
  f.done();
});

test('Ctrl/Meta toggles selection; Shift range filters privacy; grouping emits selected IDs', () => {
  const f = fixture(); f.set([tab(1), tab(2, null, true), tab(3), tab(4), tab(5, null, true)]);
  f.click(f.tab(1), { ctrlKey: true }); f.click(f.tab(4), { shiftKey: true });
  assert.deepEqual([...f.d.querySelectorAll('.tab.selected')].map(t => Number(t.dataset.tab)), [1, 3, 4]);
  assert.equal(f.last(), 'ready', 'modified clicks do not switch the active page');
  f.click(f.tab(3), { metaKey: true });
  assert.deepEqual([...f.d.querySelectorAll('.tab.selected')].map(t => Number(t.dataset.tab)), [1, 4]);
  f.right(f.tab(4)); f.click(f.d.getElementById('island-create'));
  assert.deepEqual(JSON.parse(f.last().split(':').slice(1).join(':')), { tabs: [1, 4] });
  assert.match(f.last(), /^island-create:/);
  f.click(f.tab(2), { ctrlKey: true });
  assert.deepEqual([...f.d.querySelectorAll('.tab.selected')].map(t => Number(t.dataset.tab)), [2], 'private selection replaces incompatible selection');
  f.click(f.tab(5), { shiftKey: true });
  assert.deepEqual([...f.d.querySelectorAll('.tab.selected')].map(t => Number(t.dataset.tab)), [2, 5]);
  f.done();
});

test('selection button can group a single active tab including a hidden collapsed member', () => {
  const f = fixture(); f.set([tab(1, 8)], 1, [group(8, true)]);
  f.click(f.d.getElementById('group-tabs'));
  assert.equal(f.d.getElementById('island-create').disabled, false);
  f.click(f.d.getElementById('island-create'));
  assert.equal(f.last(), 'island-create:{"tabs":[1]}');
  f.set([]); assert.equal(f.d.getElementById('group-tabs').disabled, true);
  f.done();
});

test('move targets enforce privacy and disable no-op moves; remove only grouped IDs', () => {
  const f = fixture(); const tabs = [tab(1, 8), tab(2), tab(3, 9), tab(4, 10, true)];
  f.set(tabs, 1, [group(8), group(9), group(10, false, true)]);
  f.right(f.tab(1));
  let sel = f.d.getElementById('island-target');
  assert.deepEqual([...sel.options].map(o => o.value), ['', '8', '9']);
  assert.equal(sel.options[1].disabled, true);
  assert.equal(f.d.getElementById('island-add').disabled, true);
  sel.value = '9'; sel.dispatchEvent(new f.w.Event('change'));
  assert.equal(f.d.getElementById('island-add').disabled, false);
  f.click(f.d.getElementById('island-add'));
  assert.equal(f.last(), 'island-add:{"island":9,"tabs":[1]}');
  f.click(f.tab(2), { ctrlKey: true }); f.right(f.tab(1));
  f.click(f.d.getElementById('island-remove'));
  assert.equal(f.last(), 'island-remove:{"tabs":[1]}');
  f.key('Escape'); f.right(f.tab(2));
  assert.equal(f.d.getElementById('island-remove').disabled, true);
  f.done();
});

test('context action bar stays in bookmark row; Escape/outside close; bookmark behavior remains', () => {
  const f = fixture(); f.set([tab(1)]);
  f.w.rak.setBookmarks({ bookmarks: [{ id: 5, title: '<b>safe</b>', url: 'https://book.test', folder: null }], folders: [] });
  f.right(f.tab(1));
  assert.equal(f.d.querySelectorAll('[role="toolbar"]').length, 1);
  assert.equal(f.d.getElementById('bm').getAttribute('role'), 'toolbar');
  assert.equal(f.d.querySelector('[role="dialog"]'), null);
  f.d.getElementById('url').dispatchEvent(new f.w.Event('pointerdown', { bubbles: true }));
  assert.equal(f.d.getElementById('island-create'), null);
  assert.equal(f.d.querySelector('#bm .t').textContent, '<b>safe</b>');
  f.right(f.tab(1)); f.key('Escape');
  assert.equal(f.d.querySelectorAll('.tab.selected').length, 0);
  assert.equal(f.d.getElementById('island-create'), null);
  f.click(f.d.querySelector('#bm .chip')); assert.equal(f.last(), 'bmopen:https://book.test');
  f.click(f.d.getElementById('star'));
  f.d.getElementById('bmName').value = 'Saved'; f.click(f.d.querySelector('#bm .pill.ok'));
  assert.equal(f.last(), 'bmadd:{"title":"Saved","url":"https://example.com/1","folder":null}');
  f.w.rak.setSettings({ colors: { accent: '#123456', toolbar: '#223344' } });
  assert.equal(f.d.documentElement.style.getPropertyValue('--accent'), '#123456');
  f.done();
});

test('island rename/color editor is inline, text-safe, validates empty names, and sends exact payload', () => {
  const f = fixture(); const unsafe = '<svg onload=alert(1)>';
  f.set([tab(1, 8)], 1, [group(8, false, false, unsafe)]);
  assert.equal(f.handle(8).querySelector('.island-name').textContent, unsafe);
  f.w.prompt = () => { throw new Error('native prompt must never be called'); };
  f.right(f.handle(8)); f.click(f.d.getElementById('island-edit'));
  const name = f.d.getElementById('island-name');
  assert.equal(name.closest('#bm'), f.d.getElementById('bm'));
  assert.equal(name.value, unsafe);
  name.value = '  '; name.dispatchEvent(new f.w.Event('input'));
  assert.equal(f.d.getElementById('island-save').disabled, true);
  name.value = ' <img src=x> '; name.dispatchEvent(new f.w.Event('input'));
  const color = f.d.getElementById('island-color'); color.value = '#ffd24a'; color.dispatchEvent(new f.w.Event('change'));
  f.click(f.d.getElementById('island-save'));
  assert.equal(f.last(), 'island-rename:{"id":8,"name":"<img src=x>","color":"#ffd24a"}');
  assert.equal(f.d.querySelector('img'), null);
  f.done();
});

test('island context sends new, dissolve and close commands; disappearing islands close context', () => {
  const f = fixture(); f.set([tab(1, 8)], 1, [group(8)]);
  f.right(f.d.querySelector('.island[data-island="8"]'));
  assert.ok(f.d.getElementById('island-edit'), 'group background opens island actions');
  for (const action of ['new', 'dissolve', 'close']) {
    f.right(f.handle(8)); f.click(f.d.getElementById('island-' + action)); assert.equal(f.last(), `island-${action}:8`);
  }
  f.right(f.handle(8)); f.set([tab(1)], 1);
  assert.equal(f.d.getElementById('island-edit'), null);
  f.done();
});

test('selection and context are cleaned after tab removal, including all selected tabs', () => {
  const f = fixture(); f.set([tab(1), tab(2), tab(3)]);
  f.click(f.tab(1), { ctrlKey: true }); f.click(f.tab(2), { ctrlKey: true }); f.right(f.tab(1));
  f.set([tab(2), tab(3)], 2);
  assert.equal(f.d.getElementById('selection-count').textContent, '1');
  f.click(f.d.getElementById('island-create')); assert.equal(f.last(), 'island-create:{"tabs":[2]}');
  f.right(f.tab(2)); f.set([tab(3)], 3);
  assert.equal(f.d.getElementById('island-create'), null);
  assert.equal(f.d.getElementById('selection-count').textContent, '');
  f.done();
});

test('DnD ungrouped tabs creates island; grouped targets/handles join; private mismatch rejected', () => {
  const f = fixture(); f.set([tab(1), tab(2), tab(3, 8), tab(4, 9, true)], 1, [group(8), group(9, false, true)]);
  f.drag(f.tab(1), f.tab(2)); assert.equal(f.last(), 'island-create:{"tabs":[2,1]}');
  f.drag(f.tab(1), f.tab(3)); assert.equal(f.last(), 'island-add:{"island":8,"tabs":[1]}');
  f.drag(f.tab(1), f.handle(8)); assert.equal(f.last(), 'island-add:{"island":8,"tabs":[1]}');
  const count = f.messages.length;
  f.drag(f.tab(1), f.tab(4), false); f.drag(f.tab(1), f.handle(9), false);
  f.drag(f.tab(1), f.tab(1), false); f.drag(f.tab(3), f.handle(8), false);
  assert.equal(f.messages.length, count);
  f.done();
});

test('DnD uses same-privacy selected IDs and can join collapsed handles', () => {
  const f = fixture(); f.set([tab(1), tab(2), tab(3, 8)], 1, [group(8, true)]);
  f.click(f.tab(1), { ctrlKey: true }); f.click(f.tab(2), { ctrlKey: true });
  f.drag(f.tab(1), f.handle(8));
  assert.equal(f.last(), 'island-add:{"island":8,"tabs":[1,2]}');
  f.done();
});

test('UI-scope shortcuts work in URL input and cycle tabs including hidden collapsed members', () => {
  const f = fixture(); f.set([tab(1), tab(2, 8), tab(3, 8)], 2, [group(8, true)]);
  const input = f.d.getElementById('url'); input.focus();
  for (const [key, options, message] of [
    ['t', { ctrlKey: true }, 'new'], ['N', { ctrlKey: true, shiftKey: true }, 'newp'],
    ['w', { ctrlKey: true }, 'close:2'], ['Tab', { ctrlKey: true }, 'switch:3'],
    ['Tab', { ctrlKey: true, shiftKey: true }, 'switch:1'], ['t', { altKey: true }, 'island-new:8'],
    ['t', { metaKey: true }, 'new']
  ]) { assert.equal(f.key(key, options, input).defaultPrevented, true); assert.equal(f.last(), message); }
  f.set([tab(1)], 1); const before = f.messages.length;
  assert.equal(f.key('t', { altKey: true }, input).defaultPrevented, false);
  assert.equal(f.messages.length, before);
  f.done();
});

test('keyboard tab roles: arrows focus, Enter switches, Delete closes, context key opens actions', () => {
  const f = fixture(); f.set([tab(1), tab(2), tab(3, 8)], 1, [group(8, true)]);
  f.tab(1).focus(); f.key('ArrowRight', {}, f.tab(1)); assert.equal(f.d.activeElement, f.tab(2));
  f.key('Enter', {}, f.tab(2)); assert.equal(f.last(), 'switch:2');
  f.key('Delete', {}, f.tab(2)); assert.equal(f.last(), 'close:2');
  f.key('F10', { shiftKey: true }, f.tab(2)); assert.ok(f.d.getElementById('island-create'));
  f.key('Escape'); f.handle(8).focus();
  f.key('ContextMenu', {}, f.handle(8)); assert.ok(f.d.getElementById('island-edit'));
  f.done();
});

test('title/URL updates preserve manual scroll position and tab focus; activation reveals offscreen tab', () => {
  const f = fixture(); const tabs = Array.from({ length: 12 }, (_, i) => tab(i + 1));
  f.set(tabs, 1); const scroll = f.d.getElementById('tab-scroll');
  Object.defineProperty(scroll, 'clientWidth', { configurable: true, value: 500 });
  Object.defineProperty(scroll, 'scrollWidth', { configurable: true, value: 2100 });
  scroll.getBoundingClientRect = () => ({ left: 100, right: 600 });
  // Geometry stubs survive tab reconstruction and allow meaningful reveal assertions.
  const original = f.w.HTMLElement.prototype.getBoundingClientRect;
  f.w.HTMLElement.prototype.getBoundingClientRect = function () {
    if (this.classList.contains('tab')) { const left = 100 + (Number(this.dataset.tab) - 1) * 168 - scroll.scrollLeft; return { left, right: left + 164 }; }
    return original.call(this);
  };
  scroll.scrollLeft = 510; f.tab(1).focus({ preventScroll: true });
  f.set(tabs.map(t => ({ ...t, title: t.title + ' updated', url: t.url + '?v=1' })), 1);
  assert.equal(scroll.scrollLeft, 510); assert.equal(f.d.activeElement, f.tab(1));
  f.set(tabs, 12); assert.ok(scroll.scrollLeft > 510, 'activation reveals offscreen tab');
  const position = scroll.scrollLeft; f.set(tabs.map(t => ({ ...t, title: 'new title' })), 12);
  assert.equal(scroll.scrollLeft, position);
  f.click(f.d.getElementById('scroll-left')); assert.ok(scroll.scrollLeft < position);
  f.done();
});

test('collapsing the focused active tab moves focus and reveal to active group handle', () => {
  const f = fixture(); f.set([tab(1), tab(2, 8)], 2, [group(8)]); f.tab(2).focus();
  const scroll = f.d.getElementById('tab-scroll'); scroll.getBoundingClientRect = () => ({ left: 0, right: 400 });
  const original = f.w.HTMLElement.prototype.getBoundingClientRect;
  f.w.HTMLElement.prototype.getBoundingClientRect = function () {
    if (this.classList.contains('island-handle')) return { left: 800, right: 900 };
    return original.call(this);
  };
  f.set([tab(1), tab(2, 8)], 2, [group(8, true)]);
  assert.equal(f.d.activeElement, f.handle(8)); assert.equal(scroll.scrollLeft, 504);
  const position = scroll.scrollLeft; f.set([tab(1), tab(2, 8, false, 'updated')], 2, [group(8, true)]);
  assert.equal(scroll.scrollLeft, position);
  f.done();
});

test('bookmark editor and inline island drafts survive unrelated native updates', () => {
  const f = fixture(); f.set([tab(1, 8)], 1, [group(8)]); f.click(f.d.getElementById('star'));
  let input = f.d.getElementById('bmName'); input.value = 'Unsaved'; input.focus();
  f.set([tab(1, 8, false, 'title update')], 1, [group(8)]);
  assert.equal(f.d.getElementById('bmName'), input); assert.equal(input.value, 'Unsaved');
  f.right(f.handle(8)); f.click(f.d.getElementById('island-edit'));
  input = f.d.getElementById('island-name'); input.value = 'Draft'; input.dispatchEvent(new f.w.Event('input'));
  f.set([tab(1, 8, false, 'another update')], 1, [group(8)]);
  assert.equal(f.d.getElementById('island-name'), input); assert.equal(input.value, 'Draft');
  f.done();
});

test('standalone preview instructs RAK.exe/start.bat rather than Cargo; legacy two-argument API tolerated', () => {
  const f = fixture(false); assert.equal(f.d.getElementById('url').disabled, true);
  assert.match(f.d.getElementById('url').placeholder, /RAK\.exe.*start\.bat/);
  assert.doesNotMatch(f.d.getElementById('url').placeholder, /cargo/i);
  f.w.rak.setTabs([tab(1)], 1); assert.ok(f.tab(1)); f.done();
});
