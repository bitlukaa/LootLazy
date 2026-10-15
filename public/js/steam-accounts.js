(() => {
 const el = id => document.getElementById(id);
 const model = {accounts:[],selected:null,loaded:false,busy:false,polling:false,searchSequence:0,searchTimer:null,names:{},signature:'',libraryFor:null,libraryLoading:false};
 try { model.names=JSON.parse(localStorage.getItem('lootlazy_account_games')||'{}'); } catch { model.names={}; }
 const message = error => { const key=String(error?.message||error).split(':')[0].trim(); return t(key.startsWith('accounts_')&&I18N.en[key]?key:'accounts_failed'); };
 function showError(error) { el('accounts-error').textContent=message(error);el('accounts-error').hidden=false;if(el('account-verify-dialog').open){el('account-verify-error').textContent=message(error);el('account-verify-error').hidden=false;} }
 function selected(){return model.accounts.find(account=>account.id===model.selected);}
 async function call(command,body={}) {
  if(model.busy)return false;
  model.busy=true;el('accounts-error').hidden=true;el('account-verify-error').hidden=true;render();
  try { const response=await request('/api/steam-accounts/'+command,body);apply(response);return true; }
  catch(error){showError(error);return false;}
  finally{model.busy=false;render();}
 }
 function apply(response) {
  model.accounts=Array.isArray(response?.accounts)?response.accounts:[];model.loaded=true;
  if(model.accounts.some(account=>account.connected))window.lootlazyFeatures?.markUsed('steam-accounts-v1');
  if(!model.accounts.some(account=>account.id===model.selected))model.selected=model.accounts[0]?.id||null;
  render();
 }
 let qrAccount=null,qrSequence=0,qrBusy=false;
 async function closeQr(stop=true){
  const id=qrAccount;qrAccount=null;++qrSequence;el('account-qr-code').replaceChildren();delete el('account-qr-code').dataset.signature;if(el('account-qr-dialog').open)el('account-qr-dialog').close();
  if(stop&&id&&model.accounts.some(a=>a.id===id&&a.active))return await call('stop',{id});return true;
 }
 async function readQr(){
  if(!qrAccount||qrBusy||model.busy)return;const id=qrAccount,sequence=qrSequence;qrBusy=true;
  try{const result=await request('/api/steam-accounts/qr',{id});if(sequence!==qrSequence||id!==qrAccount)return;
   const rows=result.rows;
   if(Array.isArray(rows)&&rows.length>=21&&rows.length<=97&&rows.every(row=>typeof row==='string'&&row.length===rows.length&&/^[01]+$/.test(row))){
    const signature=rows.join('');if(el('account-qr-code').dataset.signature!==signature){
     const ns='http://www.w3.org/2000/svg',svg=document.createElementNS(ns,'svg'),background=document.createElementNS(ns,'rect'),path=document.createElementNS(ns,'path');const size=rows.length+8;
     svg.setAttribute('viewBox','0 0 '+size+' '+size);svg.setAttribute('role','img');svg.setAttribute('aria-label',t('accounts_qr_signin'));svg.setAttribute('shape-rendering','crispEdges');background.setAttribute('width',String(size));background.setAttribute('height',String(size));background.setAttribute('fill','#fff');let d='';rows.forEach((row,y)=>[...row].forEach((bit,x)=>{if(bit==='1')d+='M'+(x+4)+' '+(y+4)+'h1v1h-1z';}));path.setAttribute('d',d);path.setAttribute('fill','#000');svg.append(background,path);el('account-qr-code').replaceChildren(svg);el('account-qr-code').dataset.signature=signature;
    }el('account-qr-status').textContent=t('accounts_qr_wait');
   }else{el('account-qr-code').replaceChildren();delete el('account-qr-code').dataset.signature;el('account-qr-status').textContent=t('accounts_qr_loading');}
  }catch(error){if(sequence===qrSequence){el('account-qr-code').replaceChildren();delete el('account-qr-code').dataset.signature;el('account-qr-status').textContent=message(error);}}finally{qrBusy=false;}
 }
 async function startQr(id){qrAccount=id;++qrSequence;delete el('account-qr-code').dataset.signature;el('account-qr-code').replaceChildren();el('account-qr-status').textContent=t('accounts_qr_loading');el('account-qr-dialog').showModal();if(await call('signin',{id,method:'qr'})){if(qrAccount===id)readQr();else await call('stop',{id});}else if(qrAccount===id)closeQr(false);}
 function openPassword(){const account=selected();if(!account)return;el('account-signin-form').reset();el('account-login-name-field').hidden=!!account.login;el('account-login-name').required=!account.login;el('account-login-dialog').showModal();el(account.login?'account-password':'account-login-name').focus();}
 async function poll() {
  if(!model.loaded||model.busy||model.polling||document.hidden)return;
  model.polling=true;
  try{apply(await request('/api/steam-accounts/status'));if(model.libraryLoading&&selected()?.connected)search();if(qrAccount){const account=model.accounts.find(a=>a.id===qrAccount);if(account?.connected)closeQr(false);else if(!account?.active){el('account-qr-code').replaceChildren();delete el('account-qr-code').dataset.signature;el('account-qr-status').textContent=t('accounts_qr_expired');}else readQr();}}catch(error){showError(error);}finally{model.polling=false;}
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
  el('account-auth-actions').hidden=account.active&&!account.input;el('account-signin-open').hidden=account.active;el('account-qr-open').hidden=account.active;el('account-confirm-open').hidden=!account.input;el('account-game-setup').hidden=!account.connected;
  if(!account.input&&el('account-verify-dialog').open)el('account-verify-dialog').close();
  el('account-start').hidden=!account.connected||isRunning;el('account-stop').hidden=!account.active;
  el('account-running-time').hidden=!isRunning;el('account-running-time').textContent=clock(account.elapsedSeconds||0);
  el('account-duration').disabled=model.busy||isRunning;
  el('account-search').disabled=model.busy||!gamesEditable;el('account-search').closest('.search-field').hidden=isRunning;el('account-library-hint').textContent=t('accounts_library_ready_hint');el('account-library-hint').hidden=isRunning;el('account-quick-games').hidden=!gamesEditable||!el('account-quick-picks').querySelector('button');
  for(const id of ['account-qr-open','account-signin','account-signin-open','account-confirm-open','account-submit-input','account-start','account-stop','account-remove'])el(id).disabled=model.busy;
  el('account-start').disabled=model.busy||!account.games?.length||account.state==='blocked';
  el('account-games-hint').hidden=gamesEditable||!account.connected;
  if(account.input){const type=account.input.type;el('account-input-label').textContent=t('accounts_input_'+type);el('account-code').type=['Password','SteamParentalCode'].includes(type)?'password':'text';el('account-code').autocomplete=type==='Password'?'current-password':'one-time-code';el('account-input-form').dataset.challenge=account.input.id;if(model.shownChallenge!==account.input.id&&!el('steam-accounts-mode-view').hidden){model.shownChallenge=account.input.id;el('account-verify-error').hidden=true;el('account-verify-dialog').showModal();el('account-code').focus();}}
  const gameSignature=JSON.stringify([account.id,account.games,model.names,gamesEditable,model.busy,currentLang]);
  if(el('account-games').dataset.signature!==gameSignature){
   el('account-games').dataset.signature=gameSignature;const games=document.createDocumentFragment();
   for(const id of account.games||[]){
    const row=document.createElement('div');row.className='account-game-row';const name=document.createElement('span');name.textContent=model.names[id]||'Steam App '+id;
    const remove=button(t('accounts_remove_game'),'x',()=>call('games',{id:account.id,games:account.games.filter(value=>value!==id)}),'icon-button');remove.setAttribute('aria-label',t('accounts_remove_game')+': '+name.textContent);remove.disabled=model.busy||!gamesEditable;row.append(name,remove);games.append(row);
   }el('account-games').replaceChildren(games);el('account-no-games').hidden=!!account.games?.length;
  }
  for(const control of el('account-game-setup').querySelectorAll('#account-search-results button,#account-quick-picks button'))control.disabled=model.busy||!gamesEditable;
  if(gamesEditable&&model.libraryFor!==account.id&&!model.busy){model.libraryFor=account.id;queueMicrotask(()=>search());}
 }
 async function search(refresh=false){
  const account=selected(),query=el('account-search').value.trim(),sequence=++model.searchSequence;
  if(!account?.connected||account.state==='running')return;
  el('account-search-results').replaceChildren();el('account-quick-picks').replaceChildren();el('account-quick-games').hidden=true;
  try{
   const result=await request('/api/steam-accounts/library',{id:account.id,query,refresh});
   const games=result.games;model.libraryLoading=!!result.loading;
   if(result.error)throw Error(result.error);
   if(sequence!==model.searchSequence||selected()?.id!==account.id)return;
   const fragment=document.createDocumentFragment();
   const available=(games||[]).filter(game=>{const id=Number(game.steamId||game.id);return Number.isInteger(id)&&id>0&&!account.games?.includes(id);});
   const choices=query?available:[...available].sort((a,b)=>{const rank=id=>[730,570,440].includes(Number(id))?[730,570,440].indexOf(Number(id)):3;return rank(a.steamId||a.id)-rank(b.steamId||b.id);}).slice(0,3);
   for(const game of choices){const id=Number(game.steamId||game.id);
    const row=button(game.name,'plus',async()=>{if(model.busy)return;model.names[id]=game.name;localStorage.setItem('lootlazy_account_games',JSON.stringify(model.names));if(await call('games',{id:account.id,games:[...(selected()?.games||[]),id]})){el('account-search').value='';search();}},query?'account-search-result':'button button-secondary account-quick-pick');fragment.append(row);
   }
   if(!fragment.childNodes.length&&(query||model.libraryLoading||!(games||[]).length)){const empty=document.createElement('p');empty.className='muted';empty.textContent=t(model.libraryLoading?'accounts_library_loading':query?'accounts_no_results':'accounts_library_empty');fragment.append(empty);}
   if(!query&&choices.length){el('account-quick-picks').replaceChildren(fragment);el('account-quick-games').hidden=false;}else el('account-search-results').replaceChildren(fragment);render();
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
  el('accounts-add').onclick=()=>{el('account-add-form').reset();setAddMethod();el('account-add-dialog').showModal();el('account-new-label').focus();};
  for(const [dialog,form]of [['account-add-dialog','account-add-form'],['account-login-dialog','account-signin-form'],['account-verify-dialog','account-input-form']]){
   el(dialog).addEventListener('close',()=>el(form).reset());
   const prefix=dialog.replace('-dialog','');for(const suffix of ['close','cancel']){const control=el(prefix+'-'+suffix);control.onclick=()=>el(dialog).close();if(suffix==='close')control.innerHTML=getIcon('x');}
  }
  function setAddMethod(){const qr=el('account-new-method').value==='qr';el('account-new-password-hint').hidden=qr;for(const kind of ['login','password']){el('account-new-'+kind+'-field').hidden=qr;el('account-new-'+kind).required=!qr;el('account-new-'+kind).value='';}}
  el('account-new-method').onchange=setAddMethod;
  el('account-add-form').onsubmit=async event=>{event.preventDefault();const method=el('account-new-method').value,label=el('account-new-label').value.trim(),login=method==='qr'?'':el('account-new-login').value.trim(),password=el('account-new-password').value;el('account-new-password').value='';el('account-add-dialog').close();const existing=new Set(model.accounts.map(account=>account.id));if(await call('add',{label,login,method})){const account=model.accounts.find(value=>!existing.has(value.id));if(account){model.selected=account.id;render();if(method==='qr')await startQr(account.id);else await call('signin',{id:account.id,password});}}};
  el('account-signin-open').onclick=openPassword;
  el('account-qr-open').onclick=()=>{if(selected())startQr(selected().id);};
  for(const id of ['account-qr-close','account-qr-cancel'])el(id).onclick=()=>closeQr();el('account-qr-close').innerHTML=getIcon('x');
  el('account-qr-dialog').addEventListener('cancel',event=>{event.preventDefault();closeQr();});
  el('account-qr-password').onclick=async()=>{if(await closeQr())openPassword();};
  el('account-confirm-open').onclick=()=>{el('account-verify-error').hidden=true;el('account-verify-dialog').showModal();el('account-code').focus();};
  el('accounts-retry').onclick=()=>{model.loaded=false;open();};
  el('account-signin-form').onsubmit=async event=>{event.preventDefault();const account=selected();if(!account)return;const password=el('account-password').value,login=el('account-login-name').value.trim();el('account-password').value='';el('account-login-dialog').close();await call('signin',{id:account.id,password,login});};
  el('account-input-form').onsubmit=async event=>{event.preventDefault();const account=selected();if(!account)return;const value=el('account-code').value,challengeId=el('account-input-form').dataset.challenge;el('account-code').value='';await call('input',{id:account.id,challengeId,value});};
  el('account-start').onclick=()=>{const account=selected();if(account)call('start',{id:account.id,minutes:Number(el('account-duration').value)});};
  el('account-stop').onclick=()=>{const account=selected();if(account)call('stop',{id:account.id});};
  el('account-remove').onclick=()=>{if(selected())el('account-remove-dialog').showModal();};
  el('account-remove-cancel').onclick=()=>el('account-remove-dialog').close();
  el('account-remove-confirm').onclick=async()=>{const account=selected();el('account-remove-dialog').close();if(account)await call('remove',{id:account.id});};
  el('account-search').oninput=()=>{clearTimeout(model.searchTimer);++model.searchSequence;model.searchTimer=setTimeout(()=>search(),300);};
  for(const [id,icon]of [['accounts-add','plus'],['accounts-retry','refresh'],['account-start','play'],['account-stop','stop'],['account-remove','x'],['account-signin','play'],['account-signin-open','play']]){const span=document.createElement('span');span.innerHTML=getIcon(icon);el(id).prepend(span);}
  el('account-refresh-library').onclick=()=>search(true);el('account-refresh-library').innerHTML=getIcon('refresh');
  el('account-search-icon').innerHTML=getIcon('search');el('steam-mode-accounts-icon').innerHTML=getIcon('users');
  setInterval(poll,3000);
 });
 window.lootlazySteamAccounts={open,translate:translateAccounts,async stopAll(){for(const account of [...model.accounts])if(account.active&&!await call('stop',{id:account.id}))return false;return true;},activeCount:()=>model.accounts.filter(account=>account.state==='running').length};
})();
