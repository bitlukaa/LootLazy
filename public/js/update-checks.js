(() => {
 const interval=5*60*1000;
 let lastAttempt=Date.now();
 function check(force=false){if(document.hidden||!force&&Date.now()-lastAttempt<60000)return;lastAttempt=Date.now();loadUpdates(true,true);}
 document.addEventListener('DOMContentLoaded',()=>{setInterval(()=>check(),interval);window.addEventListener('focus',()=>check());window.addEventListener('online',()=>check(true));document.addEventListener('visibilitychange',()=>{if(!document.hidden)check();});});
})();
