import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';

const source = readFileSync(new URL('../src/app.js', import.meta.url), 'utf8');
const progress = source.slice(source.indexOf('function showFiveHourProgress('), source.indexOf('function startFiveHourWindow('));
function setup() {
  const intervals = new Map(), timeouts = new Map(), rendered = [];
  let next = 0, posts = 0;
  const ctx = {
    currentLanguage: 'zh-cn', fiveHourStatusText: null, fiveHourStatusRestoreTimer: null,
    fiveHourConfirmedProfiles: new Set(), statusIdleTimer: null, statusLastOperationAt: 0,
    statusIdleDelayMs: 5000, showTiboPost: true, confirmDialogOpen: false, console,
    els: { statusText: { removeAttribute() {} } },
    renderStatusTicker(label, text) { rendered.push(text); },
    getTiboTickerDuration: () => 6, t: x => x,
    showLatestTiboPost() { posts++; }, renderOperationStatus() {},
    invokeBackend: () => Promise.resolve('cli_spawned'),
    window: {
      setInterval(fn) { intervals.set(++next, fn); return next; },
      clearInterval(id) { intervals.delete(id); },
      setTimeout(fn) { timeouts.set(++next, fn); return next; },
      clearTimeout(id) { timeouts.delete(id); }
    }
  };
  vm.createContext(ctx);
  vm.runInContext(progress, ctx);
  return { ctx, intervals, timeouts, rendered, posts: () => posts };
}

test('nonmodal progress survives polling and restores Post after a confirmed result', async () => {
  const p = setup();
  const finish = p.ctx.showFiveHourProgress('Account', 'request');
  assert.equal(p.ctx.confirmDialogOpen, false);
  await [...p.intervals.values()][0]();
  await new Promise(resolve => setImmediate(resolve));
  assert.match(p.rendered.at(-1), /正在连接/);
  const count = p.rendered.length;
  await [...p.intervals.values()][0]();
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(p.rendered.length, count, 'unchanged progress must not restart scrolling');
  finish({ confirmed: true, usage: { primaryResetAt: 1800000000 } }, null);
  assert(p.ctx.fiveHourConfirmedProfiles.has('Account'));
  assert.equal(p.intervals.size, 0);
  assert.equal(p.posts(), 0);
  [...p.timeouts.values()][0]();
  assert.equal(p.ctx.fiveHourStatusText, null);
  assert.equal(p.posts(), 1);
});

test('failed and unconfirmed results never add a success dot', () => {
  for (const [result, error] of [[{confirmed:false}, null], [null, 'Network timeout']]) {
    const p = setup();
    p.ctx.showFiveHourProgress('Account', 'request')(result, error);
    assert.equal(p.ctx.fiveHourConfirmedProfiles.size, 0);
    assert.match(p.rendered.at(-1), /失败|尚未确认/);
    assert.match(p.rendered.at(-1), /request/);
  }
});

test('new progress cancels restoration from the preceding request', () => {
  const p = setup();
  p.ctx.showFiveHourProgress('Account', 'one')({confirmed:true}, null);
  assert.equal(p.timeouts.size, 1);
  p.ctx.showFiveHourProgress('Other', 'two');
  assert.equal(p.timeouts.size, 0);
});

test('selecting a profile configuration clears only that profile success marker', () => {
  const p = setup();
  Object.assign(p.ctx, { scheduleSelectionClear(){}, renderProfiles(){}, renderConfig(){} });
  vm.runInContext(source.slice(source.indexOf('function renderProfileConfig('), source.indexOf('function adminProductLabel(')), p.ctx);
  p.ctx.fiveHourConfirmedProfiles.add('Account');
  p.ctx.fiveHourConfirmedProfiles.add('Other');
  p.ctx.renderProfileConfig('Account');
  assert.equal(p.ctx.fiveHourConfirmedProfiles.has('Account'), false);
  assert.equal(p.ctx.fiveHourConfirmedProfiles.has('Other'), true);
});
