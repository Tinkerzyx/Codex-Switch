import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';

const source = readFileSync(new URL('../src/app.js', import.meta.url), 'utf8');
const switchFunction = source.slice(source.indexOf('function switchProfile('), source.indexOf('function startLoginWatch('));

test('rapid switch requests cannot overlap, and errors release the switch guard', async () => {
  for (const failure of [false, true]) {
    let finish;
    const calls = [];
    const ctx = {
      profileSwitchRunning: false,
      fiveHourConfirmedProfiles: new Set(),
      scheduleSelectionClear() {}, renderProfiles() {}, showToast() {}, setStatus() {},
      runBackendAction(command, args) {
        calls.push({ command, args });
        return new Promise((resolve, reject) => { finish = failure ? reject : resolve; });
      }
    };
    vm.createContext(ctx);
    vm.runInContext(switchFunction, ctx);
    ctx.switchProfile('A', true);
    ctx.switchProfile('B', true);
    assert.equal(ctx.profileSwitchRunning, true);
    assert.equal(calls.length, 1);
    assert.equal(calls[0].args.profileName, 'A');
    finish(failure ? new Error('failed') : {});
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(ctx.profileSwitchRunning, false);
    ctx.switchProfile('B', true);
    assert.equal(calls.length, 2);
    finish(failure ? new Error('failed') : {});
    await new Promise(resolve => setImmediate(resolve));
  }
});
