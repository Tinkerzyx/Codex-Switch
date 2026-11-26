import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
const source=readFileSync(new URL('../src/app.js',import.meta.url),'utf8');
const request=source.slice(source.indexOf('function runFiveHourRequest('),source.indexOf('function startFiveHourWindow('));

test('automatic tasks reuse the manual backend route and release the running flag on both outcomes', async()=>{
  for(const failure of [false,true]){
    const calls=[],finished=[],applied=[],ledger=[];
    const profile={name:'A',accountId:'account-a',active:true};
    const result={confirmed:true,usage:{profileName:'A',primaryResetAt:1800000000}};
    const ctx={fiveHourRequestRunning:false,getProfileByName:()=>profile,
      autoFiveHourScheduler:{markStarted(){ledger.push('start');},recordResult(){ledger.push('result');}},
      showFiveHourProgress:()=>((r,e)=>finished.push({r,e})),renderProfiles(){},
      applyProfileUsage:(usage,options)=>applied.push({usage,options}),console,
      invokeBackend:(name,args)=>{calls.push({name,args});return name==='start_five_hour_window'?(failure?Promise.reject(new Error('offline')):Promise.resolve(result)):Promise.resolve();}
    };
    vm.createContext(ctx);vm.runInContext(request,ctx);
    const pending=ctx.runFiveHourRequest('A','request-id',true);
    assert.equal(ctx.fiveHourRequestRunning,true);
    if(failure)await assert.rejects(pending,/offline/);else assert.equal(await pending,result);
    assert.equal(ctx.fiveHourRequestRunning,false);
    assert.equal(calls.filter(c=>c.name==='start_five_hour_window').length,1);
    assert.equal(calls.find(c=>c.name==='start_five_hour_window').args.requestId,'request-id');
    assert.equal(calls[0].args.stage,'automatic_started');
    assert.equal(finished.length,1);
    assert.equal(ledger.length,0,'automatic ledger is owned by the serial scheduler');
    if(!failure)assert.equal(applied[0].options.fiveHourResult,true,'CLI result must not become a fresh polling observation');
  }
});

test('shared runner prevents overlapping calls and preserves the manual active-account restriction', async()=>{
  for(const busy of [true,false]){
    const ctx={fiveHourRequestRunning:busy,getProfileByName:()=>({active:true})};
    vm.createContext(ctx);vm.runInContext(request,ctx);
    await assert.rejects(ctx.runFiveHourRequest('A','id',false),busy?/already running/:/no longer available/);
  }
});
