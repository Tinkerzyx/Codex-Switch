import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
const source = readFileSync(new URL('../src/app.js', import.meta.url), 'utf8');
const functions = source.slice(source.indexOf('function refreshResetIndicator('), source.indexOf('function updateStatusHeight('));
const now = 1791700000;
function setup(result, error, saved = '') {
 const states = [], calls = [], timers = new Map(), handlers = {}, storage = new Map([['reset', saved]]);
 const indicator = { addEventListener(name, fn) { handlers[name] = fn; }, setPointerCapture() {} };
 let next = 0;
 const ctx = { showTiboPost: false, latestTiboPost: null, tiboRefreshTimer: null, tiboRefreshIntervalMs: 300000, statusIdleTimer: null,
  resetPostAt: 0, resetPostStorageKey: 'reset', codexResetStatus: 'unknown', Date: { now: () => now * 1000 },
  renderCodexResetStatus(s) { states.push(s); ctx.codexResetStatus=s; }, getTauriInvoke: () => command => { calls.push(command); return error ? Promise.reject(error) : Promise.resolve(result); },
  saveCachedTiboPost() {}, showLatestTiboPostIfIdle() {}, loadCachedTiboPost() {}, renderOperationStatus() {}, t: x => x,
  saveBooleanSetting() {}, showTiboPostStorageKey: '', showLatestTiboPost() {}, console: { warn() {} }, els: { resetStatusIndicator: indicator }, setStatus() {},
  window: { localStorage: { getItem: key => storage.get(key), setItem: (key,value) => storage.set(key,value) },
   setInterval(fn, ms) { timers.set(++next,{fn,ms}); return next; }, clearInterval(id) { timers.delete(id); },
   setTimeout(fn,ms) { timers.set(++next,{fn,ms}); return next; }, clearTimeout(id) { timers.delete(id); }, addEventListener() {} }
 };
 vm.createContext(ctx); vm.runInContext(functions, ctx);
 return { ctx, states, calls, timers, handlers, storage };
}
test('history signal lights lamp while latest unrelated tweet is still displayed', async () => {
 const latestPost = { text: 'Other news' };
 const p = setup({ latestPost, resetPostAt: now - 3600 });
 await p.ctx.refreshLatestTiboPost();
 assert.deepEqual(p.calls, ['get_tibo_timeline']);
 assert.deepEqual(p.states, ['confirmed']);
 assert.equal(p.ctx.latestTiboPost, latestPost);
 p.ctx.updateResetIndicatorFromTimeline({ resetPostAt: null });
 assert.equal(p.states.at(-1),'confirmed');
});
test('signal expires at original post age of 24h even without a new request', () => {
 const p = setup();
 p.ctx.updateResetIndicatorFromTimeline({resetPostAt:now-86399});
 assert.equal(p.states.at(-1),'confirmed');
 p.ctx.Date.now = () => (now+1)*1000;
 p.ctx.refreshResetIndicator();
 assert.equal(p.states.at(-1),'unknown');
 for(const timestamp of [now-86400,now+10,'invalid']){
  const p=setup(); p.ctx.updateResetIndicatorFromTimeline({resetPostAt:timestamp});
  assert.equal(p.states.at(-1),'unknown');
 }
});
test('remembered signal survives restart and network failure, expired signal does not', async () => {
 for(const [saved,expected] of [[now-100,'confirmed'],[now-86400,'unknown'],[now+10,'unknown']]){
  const p=setup(null,new Error('offline'),String(saved));
  p.ctx.startResetHistoryIndicator();
  await p.ctx.refreshLatestTiboPost();
  assert.equal(p.states.at(-1),expected);
 }
});
test('hiding ticker preserves polling', () => {
 const p = setup({latestPost:{text:'News'}});
 p.ctx.startTiboPostTicker();
 const timer=p.ctx.tiboRefreshTimer;
 p.ctx.setTiboPostVisibility(false);
 assert.equal(p.ctx.tiboRefreshTimer,timer);
 assert.equal(p.timers.get(timer).ms,300000);
});
function down(p) { p.handlers.pointerdown({button:0,isPrimary:true,clientX:10,clientY:10,pointerId:1,preventDefault(){},stopPropagation(){}}); }
test('holding green lamp opens default-browser command exactly once after 700ms', () => {
 const p=setup(); p.ctx.codexResetStatus='confirmed';p.ctx.setupResetIndicatorLongPress();down(p);
 const timer=[...p.timers.values()][0]; assert.equal(timer.ms,700);
 timer.fn();p.handlers.pointerup();
 assert.deepEqual(p.calls,['open_tibo_timeline']);
});
test('short press, drag, cancellation and unlit lamp never open browser', () => {
 for(const event of ['pointerup','pointercancel','lostpointercapture','blur','pointermove']) {
  const p=setup();p.ctx.codexResetStatus='confirmed';p.ctx.setupResetIndicatorLongPress();down(p);
  p.handlers[event]({clientX:40,clientY:10});
  assert.equal(p.timers.size,0);assert.deepEqual(p.calls,[]);
 }
 const p=setup();p.ctx.setupResetIndicatorLongPress();down(p);assert.equal(p.timers.size,0);
});
