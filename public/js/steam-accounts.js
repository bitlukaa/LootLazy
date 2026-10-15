(() => {
 const el = id => document.getElementById(id);
 const model = {accounts:[],selected:null,loaded:false,busy:false,polling:false,searchSequence:0,searchTimer:null,names:{},signature:'',libraryFor:null,libraryLoading:false};
 try { model.names=JSON.parse(localStorage.getItem('lootlazy_account_games')||'{}'); } catch { model.names={}; }
 const message = error => { const key=String(error?.message||error).split(':')[0].trim(); return t(key.startsWith('accounts_')&&I18N.en[key]?key:'accounts_failed'); };
 function showError(error) { el('accounts-error').textContent=message(error);el('accounts-error').hidden=false; }
 function selected(){return model.accounts.find(account=>account.id===model.selected);}
 async function call(command,body={}) {
  if(model.busy)return false;
  model.busy=true;el('accounts-error').hidden=true;render();
  try { const response=await request('/api/steam-accounts/'+command,body);apply(response);return true; }
  catch(error){showError(error);return false;}
  finally{model.busy=false;render();}
 }
 function apply(response) {
  model.accounts=Array.isArray(response?.accounts)?response.accounts:[];model.loaded=true;
  if(!model.accounts.some(account=>account.id===model.selected))model.selected=model.accounts[0]?.id||null;
  render();
 }
 async function poll() {
  if(!model.loaded||model.busy||model.polling||document.hidden)return;
  model.polling=true;
  try{apply(await request('/api/steam-accounts/status'));if(model.libraryLoading&&selected()?.connected)search();}catch(error){showError(error);}finally{model.polling=false;}
 }
 async function open() {
  if(model.loaded){render();return;}
  el('accounts-loading').hidden=false;
  await call('status');el('accounts-loading').hidden=true;
 }
 function button(label,icon,action,className='button button-secondary') {
  const control=document.createElement('button');control.type='button';control.className=className;
  control.innerHTML=getIcon(icon)+(className==='icon-button'?'':'<span>'+escapeHtml(label)+'</span>');control.setAttribute('aria-label',label);control.onclick=action;return control;
 }
 function text(id,value){const node=el(id);if(node.textContent!==value)node.textContent=value;}
 function render() {
  if(!el('accounts-list'))return;
  renderActive();
  const signature=JSON.stringify([model.accounts.map(({elapsedSeconds,...account})=>account),model.selected,currentLang,model.busy]);
  if(signature!==model.signature){
   model.signature=signature;const list=document.createDocumentFragment();
   for(const account of model.accounts){
    const row=document.createElement('button');row.type='button';row.className='account-row'+(account.id===model.selected?' selected':'');row.setAttribute('aria-pressed',String(account.id===model.selected));
    const text=document.createElement('span'),name=document.createElement('strong'),login=document.createElement('small');name.textContent=account.label;login.textContent=account.login;text.append(name,login);
    const badge=document.createElement('span');badge.className='account-state '+account.state;badge.textContent=t('accounts_state_'+account.state);row.append(text,badge);
    row.onclick=()=>{model.selected=account.id;model.libraryFor=null;el('account-code').value='';el('account-password').value='';el('account-search').value='';el('account-search-results').replaceChildren();render();};list.append(row);
   }
   el('accounts-list').replaceChildren(list);el('accounts-empty').hidden=model.accounts.length>0;
  }
  el('accounts-add').disabled=model.busy;el('accounts-retry').disabled=model.busy;
  const account=selected();el('account-setup').hidden=!account;el('account-no-selection').hidden=!!account;
  if(!account)return;
  el('account-auth-error').hidden=!account.error;el('account-auth-error').textContent=account.error?t(account.error==='accounts_restore_failed'?'accounts_restore_failed':'accounts_login_failed'):'';
  text('account-title',account.label);text('account-login',account.login);
  if(el('account-state').className!=='account-state '+account.state)el('account-state').className='account-state '+account.state;text('account-state',t('accounts_state_'+account.state));
  if(!account.connected){model.libraryFor=null;model.libraryLoading=false;}
  const isRunning=account.state==='running',isWaiting=['connecting','input'].includes(account.state);
  const gamesEditable=account.connected&&!isRunning&&!isWaiting;
  el('account-refresh-library').disabled=model.busy||!gamesEditable;
  el('account-signin-form').hidden=account.active;el('account-input-form').hidden=!account.input;
  el('account-start').hidden=!account.connected||isRunning;el('account-stop').hidden=!account.active;
  el('account-running-time').hidden=!isRunning;el('account-running-time').textContent=clock(account.elapsedSeconds||0);
  el('account-duration').disabled=model.busy||isRunning;
  el('account-search').disabled=model.busy||!gamesEditable;
  for(const id of ['account-signin','account-submit-input','account-start','account-stop','account-remove'])el(id).disabled=model.busy;
  el('account-start').disabled=model.busy||!account.games?.length||account.state==='blocked';
  el('account-games-hint').hidden=gamesEditable||!account.connected;
  if(account.input){const type=account.input.type;el('account-input-label').textContent=t('accounts_input_'+type);el('account-code').type=['Password','SteamParentalCode'].includes(type)?'password':'text';el('account-code').autocomplete=type==='Password'?'current-password':'one-time-code';el('account-input-form').dataset.challenge=account.input.id;}
  const gameSignature=JSON.stringify([account.id,account.games,model.names,gamesEditable,model.busy,currentLang]);
  if(el('account-games').dataset.signature!==gameSignature){
   el('account-games').dataset.signature=gameSignature;const games=document.createDocumentFragment();
   for(const id of account.games||[]){
    const row=document.createElement('div');row.className='account-game-row';const name=document.createElement('span');name.textContent=model.names[id]||'Steam App '+id;
    const remove=button(t('accounts_remove_game'),'x',()=>call('games',{id:account.id,games:account.games.filter(value=>value!==id)}),'icon-button');remove.setAttribute('aria-label',t('accounts_remove_game')+': '+name.textContent);remove.disabled=model.busy||!gamesEditable;row.append(name,remove);games.append(row);
   }el('account-games').replaceChildren(games);el('account-no-games').hidden=!!account.games?.length;
  }
  for(const control of el('account-search-results').querySelectorAll('button'))control.disabled=model.busy||!gamesEditable;
  if(gamesEditable&&model.libraryFor!==account.id&&!model.busy){model.libraryFor=account.id;queueMicrotask(()=>search());}
 }
 async function search(refresh=false){
  const account=selected(),query=el('account-search').value.trim(),sequence=++model.searchSequence;
  if(!account?.connected||account.state==='running')return;
  el('account-search-results').replaceChildren();
  try{
   const result=await request('/api/steam-accounts/library',{id:account.id,query,refresh});
   const games=result.games;model.libraryLoading=!!result.loading;
   if(result.error)throw Error(result.error);
   if(sequence!==model.searchSequence||selected()?.id!==account.id)return;
   const fragment=document.createDocumentFragment();
   for(const game of games||[]){const id=Number(game.steamId||game.id);if(!Number.isInteger(id)||id<1||account.games?.includes(id))continue;
    const row=button(game.name,'plus',async()=>{if(model.busy)return;model.names[id]=game.name;localStorage.setItem('lootlazy_account_games',JSON.stringify(model.names));if(await call('games',{id:account.id,games:[...(selected()?.games||[]),id]})){el('account-search').value='';el('account-search-results').replaceChildren();}},'account-search-result');fragment.append(row);
   }
   if(!fragment.childNodes.length){const empty=document.createElement('p');empty.className='muted';empty.textContent=t(model.libraryLoading?'accounts_library_loading':'accounts_no_results');fragment.append(empty);}el('account-search-results').replaceChildren(fragment);render();
  }catch(error){if(sequence===model.searchSequence)showError(error);}
 }
 function renderActive(){
  const list=el('accounts-active-list');if(!list)return;
  const running=model.accounts.filter(account=>account.state==='running');el('accounts-active-panel').hidden=!running.length;window.lootlazyRotations?.refreshSteamSummary?.();
  const signature=JSON.stringify([running.map(account=>[account.id,account.label,account.games]),model.busy,currentLang]);
  if(list.dataset.signature!==signature){list.dataset.signature=signature;const fragment=document.createDocumentFragment();for(const account of running){const row=document.createElement('div');row.className='account-game-row';row.dataset.account=account.id;const label=document.createElement('strong');label.textContent=account.label;const time=document.createElement('span');time.className='clock-digits';const stop=button(t('quest_stop'),'square',()=>call('stop',{id:account.id}),'button button-danger');stop.disabled=model.busy;row.append(label,time,stop);fragment.append(row);}list.replaceChildren(fragment);}
  for(const account of running){const row=[...list.children].find(item=>item.dataset.account===account.id);if(row)row.querySelector('.clock-digits').textContent=clock(account.elapsedSeconds||0);}
 }
 function translateAccounts(){model.signature='';render();}
 document.addEventListener('DOMContentLoaded',()=>{
  el('accounts-add').onclick=()=>{el('account-add-form').reset();el('account-add-dialog').showModal();el('account-new-label').focus();};
  el('account-add-cancel').onclick=()=>el('account-add-dialog').close();
  el('account-add-form').onsubmit=async event=>{event.preventDefault();const label=el('account-new-label').value.trim(),login=el('account-new-login').value.trim();el('account-add-dialog').close();if(await call('add',{label,login}))model.selected=model.accounts.at(-1)?.id;render();};
  el('accounts-retry').onclick=()=>{model.loaded=false;open();};
  el('account-signin-form').onsubmit=async event=>{event.preventDefault();const account=selected();if(!account)return;const password=el('account-password').value;el('account-password').value='';await call('signin',{id:account.id,password});};
  el('account-input-form').onsubmit=async event=>{event.preventDefault();const account=selected();if(!account)return;const value=el('account-code').value,challengeId=el('account-input-form').dataset.challenge;el('account-code').value='';await call('input',{id:account.id,challengeId,value});};
  el('account-start').onclick=()=>{const account=selected();if(account)call('start',{id:account.id,minutes:Number(el('account-duration').value)});};
  el('account-stop').onclick=()=>{const account=selected();if(account)call('stop',{id:account.id});};
  el('account-remove').onclick=()=>{if(selected())el('account-remove-dialog').showModal();};
  el('account-remove-cancel').onclick=()=>el('account-remove-dialog').close();
  el('account-remove-confirm').onclick=async()=>{const account=selected();el('account-remove-dialog').close();if(account)await call('remove',{id:account.id});};
  el('account-search').oninput=()=>{clearTimeout(model.searchTimer);++model.searchSequence;model.searchTimer=setTimeout(()=>search(),300);};
  for(const [id,icon]of [['accounts-add','plus'],['accounts-retry','refresh'],['account-start','play'],['account-stop','stop'],['account-remove','x'],['account-signin','play']]){const span=document.createElement('span');span.innerHTML=getIcon(icon);el(id).prepend(span);}
  el('account-refresh-library').onclick=()=>search(true);el('account-refresh-library').innerHTML=getIcon('refresh');
  el('account-search-icon').innerHTML=getIcon('search');el('steam-mode-accounts-icon').innerHTML=getIcon('users');
  setInterval(poll,3000);
 });
 window.lootlazySteamAccounts={open,translate:translateAccounts,async stopAll(){for(const account of [...model.accounts])if(account.active&&!await call('stop',{id:account.id}))return false;return true;},activeCount:()=>model.accounts.filter(account=>account.state==='running').length};
})();
