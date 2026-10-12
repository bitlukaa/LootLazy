// Card-drop planning accepts only explicit, fresh counts from a future provider.
// No account access or Steam requests are performed by this module.
function planCardDrops(snapshot,{now=Date.now(),maxAgeMs=300000}={}) {
 if(!snapshot||snapshot.status!=='ready')return {status:'unavailable',games:[],remaining:null};
 const checkedAt=Date.parse(snapshot.checkedAt);
 if(!Number.isFinite(checkedAt)||checkedAt>now||now-checkedAt>maxAgeMs)return {status:'stale',games:[],remaining:null};
 if(!Array.isArray(snapshot.games))throw new TypeError('Invalid card-drop games');
 const seen=new Set(),games=[];
 for(const game of snapshot.games){
  if(!/^\d+$/.test(String(game.appId))||seen.has(String(game.appId))||!Number.isSafeInteger(game.remaining)||game.remaining<0)throw new TypeError('Invalid card-drop count');
  seen.add(String(game.appId));
  if(game.remaining>0)games.push({appId:String(game.appId),remaining:game.remaining});
 }
 return {status:'ready',games,remaining:games.reduce((sum,g)=>sum+g.remaining,0)};
}
if(typeof module!=='undefined')module.exports={planCardDrops};
