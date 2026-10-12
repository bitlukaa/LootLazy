const test=require('node:test'),assert=require('node:assert/strict');
const {planCardDrops}=require('../public/js/card-drops.js');
const now=Date.parse('2026-10-12T12:00:00Z');
const snapshot=games=>({status:'ready',checkedAt:new Date(now).toISOString(),games});
test('Missing data is unknown, never zero drops',()=>assert.deepEqual(planCardDrops(null),{status:'unavailable',games:[],remaining:null}));
test('Expired or future counts cannot drive an idle queue',()=>{for(const offset of [-300001,1])assert.equal(planCardDrops({...snapshot([]),checkedAt:new Date(now+offset).toISOString()},{now}).status,'stale');});
test('Only games with confirmed remaining drops enter the plan',()=>assert.deepEqual(planCardDrops(snapshot([{appId:'1',remaining:0},{appId:'2',remaining:3}]),{now}),{status:'ready',games:[{appId:'2',remaining:3}],remaining:3}));
test('Invalid and duplicate counts fail closed',()=>{for(const games of [[{appId:'2',remaining:-1}],[{appId:'2',remaining:1.5}],[{appId:'2',remaining:1},{appId:'2',remaining:2}]])assert.throws(()=>planCardDrops(snapshot(games),{now}),TypeError);});
