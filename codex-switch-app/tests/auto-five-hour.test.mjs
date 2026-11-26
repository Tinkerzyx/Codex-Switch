import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
const source = readFileSync(new URL('../src/auto-five-hour.js', import.meta.url), 'utf8');

function setup({ saved = {}, run, list } = {}) {
  let now = 1800000000, canRun = true;
  const profiles = list || [{name:'A',accountId:'account-a',active:false}, {name:'B',accountId:'account-b',active:false}];
  const calls = [], logs = [];
  let persisted = structuredClone(saved);
  const ctx = {};
  vm.createContext(ctx); vm.runInContext(source, ctx);
  const scheduler = ctx.createAutoFiveHourScheduler({
    now: () => now, profiles: () => profiles, canRun: () => canRun,
    load: () => structuredClone(persisted), save: state => {persisted=JSON.parse(JSON.stringify(state));},
    log: (stage, profile, details) => logs.push({stage,name:profile.name,details}),
    run: async profile => {
      calls.push(profile.name);
      return run ? run(profile) : {confirmed:true,usage:{primaryResetAt:now+18000}};
    }
  });
  scheduler.setEnabled(true);
  function observe(name='A', reset=now+18000, left=100) {
    scheduler.observe(profiles.find(p=>p.name===name), {primaryResetAt:reset,primaryLeft:left});
  }
  return { scheduler, calls, logs, profiles, observe, now:()=>now, advance:seconds=>{now+=seconds;}, block:value=>{canRun=!value;}, saved:()=>persisted };
}

test('requires two minute-spaced rolling samples, with no cached/rapid refresh trigger', async () => {
  const p=setup();p.observe();await p.scheduler.drain();assert.equal(p.calls.length,0);
  p.advance(10);p.observe();await p.scheduler.drain();assert.equal(p.calls.length,0);
  p.advance(50);p.observe();await p.scheduler.drain();assert.deepEqual(p.calls,['A']);
  assert.equal(p.logs.find(x=>x.stage==='auto_queued').details.sampleSeconds,60);
});

test('a fixed 100% reset and partially used quota do not trigger', async () => {
  for(const left of [100,99]) {
    const p=setup(),reset=p.now()+18000;p.observe('A',reset,left);p.advance(60);p.observe('A',reset,left);
    await p.scheduler.drain();assert.equal(p.calls.length,0);
  }
});

test('near-simultaneous accounts run serially and deduplicate while running', async () => {
  let release;const blocked=new Promise(resolve=>{release=resolve;});let active=0,max=0;
  const p=setup({run:async()=>{active++;max=Math.max(max,active);await blocked;active--;return {confirmed:false};}});
  p.observe('A');p.observe('B');p.advance(60);p.observe('A');p.observe('B');
  const drain=p.scheduler.drain();await p.scheduler.drain();assert.deepEqual(p.calls,['A']);
  p.advance(60);p.observe('A');release();await drain;assert.deepEqual(p.calls,['A','B']);assert.equal(max,1);
});

test('current account and its aliases use the same rolling-window rule and one queue slot', async () => {
  const p=setup({list:[{name:'A',accountId:'same',active:false},{name:'B',accountId:'same',active:true}]});
  p.observe('A');p.observe('B');p.advance(60);p.observe('B');p.observe('A');await p.scheduler.drain();assert.equal(p.calls.length,1);
});

test('current account is scheduled while rolling but never when the reset is already fixed', async () => {
  const p=setup({list:[{name:'A',accountId:'current',active:true}]});
  p.observe();p.advance(60);p.observe();await p.scheduler.drain();assert.deepEqual(p.calls,['A']);
  const fixed=setup({list:[{name:'A',accountId:'current',active:true}]});
  const reset=fixed.now()+18000;fixed.observe('A',reset);fixed.advance(60);fixed.observe('A',reset);
  await fixed.scheduler.drain();assert.equal(fixed.calls.length,0);
});

test('queued account becoming current is still eligible under the same rule', async () => {
  const p=setup();p.observe();p.advance(60);p.observe();p.profiles[0].active=true;
  await p.scheduler.drain();assert.deepEqual(p.calls,['A']);
});

test('aliases of an inactive account share one queue slot', async () => {
  const p=setup({list:[{name:'A',accountId:'same',active:false},{name:'B',accountId:'same',active:false}]});
  p.observe('A');p.observe('B');p.advance(60);p.observe('A');p.observe('B');await p.scheduler.drain();assert.equal(p.calls.length,1);
});

test('queued profile deleted or stale is revalidated before execution', async () => {
  for(const change of [p=>{p.profiles.shift();},p=>p.advance(181)]) {
    const p=setup();p.observe();p.advance(60);p.observe();change(p);await p.scheduler.drain();assert.equal(p.calls.length,0);
  }
});

test('confirmed reset persists across restart and allows a new cycle only after expiry', async () => {
  const p=setup();p.observe();p.advance(60);p.observe();await p.scheduler.drain();
  const state=p.saved(),until=state['account-a'].confirmedUntil;
  const restarted=setup({saved:state});restarted.observe();restarted.advance(60);restarted.observe();await restarted.scheduler.drain();assert.equal(restarted.calls.length,0);
  restarted.advance(until-restarted.now()+1);restarted.observe();restarted.advance(60);restarted.observe();await restarted.scheduler.drain();assert.deepEqual(restarted.calls,['A']);
});

test('failed requests back off by 1, 5, 15, then 60 minutes without an endless rapid retry loop', async () => {
  const p=setup({run:()=>{throw new Error('offline');}});
  for(const delay of [60,300,900,3600,3600]) {
    p.observe();p.advance(60);p.observe();await p.scheduler.drain();
    assert.equal(p.saved()['account-a'].retryAt-p.now(),delay);
    const count=p.calls.length;await p.scheduler.drain();assert.equal(p.calls.length,count);
    p.advance(delay);
  }
});

test('disabled switch clears pending work while the current task finishes', async () => {
  let release;const result=new Promise(resolve=>{release=resolve;});const p=setup({run:()=>result});
  p.observe('A');p.observe('B');p.advance(60);p.observe('A');p.observe('B');const pending=p.scheduler.drain();
  p.scheduler.setEnabled(false);release({confirmed:false});await pending;assert.deepEqual(p.calls,['A']);
  p.scheduler.setEnabled(true);await p.scheduler.drain();assert.deepEqual(p.calls,['A']);
});

test('stale observations after sleep require fresh samples; busy/manual operation blocks draining', async () => {
  const p=setup();p.observe();p.advance(3600);p.observe();await p.scheduler.drain();assert.equal(p.calls.length,0);
  p.advance(60);p.observe();p.block(true);await p.scheduler.drain();assert.equal(p.calls.length,0);
  p.block(false);await p.scheduler.drain();assert.deepEqual(p.calls,['A']);
});

test('a persisted in-flight lease prevents immediate duplicate execution after restart', async () => {
  const p=setup();p.scheduler.markStarted(p.profiles[0]);const restarted=setup({saved:p.saved()});
  restarted.observe();restarted.advance(60);restarted.observe();await restarted.scheduler.drain();assert.equal(restarted.calls.length,0);
});
