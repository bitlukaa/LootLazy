(() => {
 const el=id=>document.getElementById(id);
 const model={accounts:[],selected:null,loaded:false,busy:false,pending:0,epoch:0,polling:false,commands:Promise.resolve(),searchSequence:0,searchTimer:null,names:{},signature:'',libraryFor:null,libraryLoading:false,durations:{}};
 let auth=null,removalTarget=null;
 try{model.names=JSON.parse(localStorage.getItem('lootlazy_account_games')||'{}');model.selected=localStorage.getItem('lootlazy_selected_account');model.durations=JSON.parse(localStorage.getItem('lootlazy_account_durations')||'{}');}catch{model.names={};}
 const message=error=>{const key=String(error?.message||error).split(':')[0].trim();return t(key.startsWith('accounts_')&&I18N.en[key]?key:'accounts_failed');};
 function showError(error){const target=el(auth?'account-auth-error':'accounts-error');target.textContent=message(error);target.hidden=false;}
 function selected(){return model.accounts.find(account=>account.id===model.selected&&!account.draft);}
 function rememberSelection(id){model.selected=id;try{localStorage.setItem('lootlazy_selected_account',id||'');}catch{}}
 function apply(response){
  if(!Array.isArray(response?.accounts))return;
  model.accounts=response.accounts;model.loaded=true;
  if(model.accounts.some(account=>account.connected&&!account.draft))window.lootlazyFeatures?.markUsed('steam-accounts-v1');
  if(!selected())rememberSelection(model.accounts.find(account=>!account.draft)?.id||null);
  render();
 }
 function call(command,body={}){
  ++model.pending;++model.epoch;model.busy=true;render();
  const operation=model.commands.then(async()=>{
   try{const response=await request('/api/steam-accounts/'+command,body);el('accounts-error').hidden=true;apply(response);return true;}catch(error){showError(error);return false;}
   finally{model.busy=--model.pending>0;render();}
  });
  model.commands=operation.catch(()=>{});return operation;
 }
 function clearQr(){el('account-qr-code').replaceChildren();el('account-qr-frame').classList.remove('expired');el('account-qr-retry').hidden=true;if(auth)auth.qrRevision=null;}
 function expireQr(){el('account-qr-frame').classList.add('expired');el('account-qr-retry').hidden=false;el('account-qr-status').textContent=t('accounts_qr_expired_short');if(auth)auth.qrRevision=null;}
 function closeAuth(cancel=true){
  const context=auth;if(!context)return Promise.resolve(true);auth=null;context.closed=true;++context.sequence;
  el('account-auth-password').value='';el('account-guard-code').value='';el('account-auth-login').value='';el('account-auth-label').value='';el('account-qr-code').replaceChildren();
  if(el('account-auth-dialog').open)el('account-auth-dialog').close();clearQr();
  if(cancel&&context.id)return call(context.draft?'remove':'disconnect',{id:context.id});return Promise.resolve(true);
 }
 async function ensureDraft(context){
  if(context.id)return auth===context;
  const before=new Set(model.accounts.map(account=>account.id));
  if(!await call('add',{label:t('accounts_pending_label'),login:'',method:'qr',draft:true}))return false;
  context.id=model.accounts.find(account=>!before.has(account.id)&&account.draft)?.id;
  if(auth!==context){if(context.id)await call('remove',{id:context.id});return false;}
  return !!context.id;
 }
 async function disconnectAttempt(context){
  const account=model.accounts.find(account=>account.id===context.id);
  if(account?.active&&!await call('disconnect',{id:context.id}))return false;
  return auth===context;
 }
 async function startQr(context){
  if(auth!==context)return;context.method='qr';++context.sequence;clearQr();el('account-auth-error').hidden=true;el('account-qr-status').textContent=t('accounts_qr_loading');
  if(!await ensureDraft(context)||!await disconnectAttempt(context))return;
  const sequence=context.sequence;
  if(await call('signin',{id:context.id,method:'qr'})){
   if(auth===context&&sequence===context.sequence)readQr(context);
  }
 }
 async function readQr(context){
  if(auth!==context||context.method!=='qr'||context.reading||model.busy)return;
  const sequence=context.sequence;context.reading=true;
  try{
   const result=await request('/api/steam-accounts/qr',{id:context.id});
   if(auth!==context||sequence!==context.sequence||context.method!=='qr')return;
   const rows=result.rows;
   if(Array.isArray(rows)&&rows.length>=21&&rows.length<=97&&rows.every(row=>typeof row==='string'&&row.length===rows.length&&/^[01]+$/.test(row))){
    const revision=rows.join('');
    if(context.qrRevision!==revision){
     const ns='http://www.w3.org/2000/svg',svg=document.createElementNS(ns,'svg'),background=document.createElementNS(ns,'rect'),path=document.createElementNS(ns,'path'),size=rows.length+8;
     const ratio=window.devicePixelRatio||1,edge=size*Math.max(2,Math.floor(216*ratio/size))/ratio;
     svg.setAttribute('viewBox','0 0 '+size+' '+size);svg.setAttribute('role','img');svg.setAttribute('aria-label',t('accounts_qr_signin'));svg.setAttribute('shape-rendering','crispEdges');svg.style.width=edge+'px';svg.style.height=edge+'px';
     background.setAttribute('width',String(size));background.setAttribute('height',String(size));background.setAttribute('fill','#fff');
     let d='';rows.forEach((row,y)=>[...row].forEach((bit,x)=>{if(bit==='1')d+='M'+(x+4)+' '+(y+4)+'h1v1h-1z';}));path.setAttribute('d',d);path.setAttribute('fill','#000');svg.append(background,path);el('account-qr-code').replaceChildren(svg);context.qrRevision=revision;
    }
    el('account-auth-error').hidden=true;el('account-qr-frame').classList.remove('expired');el('account-qr-retry').hidden=true;el('account-qr-status').textContent=t('accounts_qr_wait');
   }else if(result.pending){el('account-qr-status').textContent=t('accounts_qr_loading');}else expireQr();
  }catch(error){if(auth===context&&sequence===context.sequence){clearQr();el('account-qr-status').textContent=message(error);}}
  finally{context.reading=false;}
 }
 function openAuth(account=null){
  if(auth||model.busy)return;
  const context=auth={id:account?.id||null,draft:!account,method:account?.active?'existing':'qr',sequence:0,reading:false,finishing:false};
  el('account-auth-form').reset();el('account-guard-form').reset();el('account-auth-error').hidden=true;clearQr();
  el('account-auth-title').textContent=t(account?'accounts_signin':'accounts_add');
  el('account-auth-label').value=account?.label||'';el('account-auth-label').readOnly=!!account;el('account-auth-label').closest('label').hidden=!!account;
  el('account-auth-login').value=account?.login||'';el('account-auth-login').readOnly=!!account?.login;
  el('account-auth-password').required=!account;el('account-auth-password').placeholder='';
  el('account-qr-status').textContent=t(account?.active?'accounts_auth_pending':'accounts_qr_loading');
  el('account-auth-dialog').showModal();el(account?'account-auth-password':'account-auth-label').focus();renderAuth();if(!account?.active)startQr(context);
 }
 async function finishAuth(context){
  if(auth!==context||context.finishing)return;context.finishing=true;renderAuth();
  const label=el('account-auth-label').value.trim();
  if(!await call('commit',{id:context.id,label})){context.finishing=false;return;}
  if(auth!==context)return;
  rememberSelection(context.id);model.libraryFor=null;await closeAuth(false);render();
 }
 function renderAuth(){
  const context=auth;if(!context)return;
  const account=model.accounts.find(item=>item.id===context.id);
  const input=account?.input;
  el('account-auth-form').hidden=!!input;el('account-guard-form').hidden=!input;
  for(const id of ['account-auth-submit','account-guard-submit','account-qr-refresh','account-qr-retry'])el(id).disabled=model.busy||context.finishing;
  if(input){
   text('account-guard-label',t('accounts_input_'+input.type));el('account-guard-code').type=['Password','SteamParentalCode'].includes(input.type)?'password':'text';
   if(context.challenge!==input.id){context.challenge=input.id;el('account-guard-code').value='';el('account-guard-code').focus();}
   clearQr();el('account-qr-status').textContent=t('accounts_auth_pending');
  }
  if(account?.error){
   if(context.method==='qr'&&['accounts_qr_failed','accounts_qr_expired'].includes(account.error)){el('account-auth-error').hidden=true;expireQr();}
   else{showError(account.error);if(context.method==='qr')expireQr();}
  }
  if(account?.connected&&!context.finishing)queueMicrotask(()=>finishAuth(context));
 }
 async function poll(){
  if(!model.loaded||model.busy||model.polling||document.hidden)return;
  model.polling=true;const epoch=model.epoch;
  try{const response=await request('/api/steam-accounts/status');if(epoch!==model.epoch)return;apply(response);if(model.libraryLoading&&selected()?.connected)search();if(auth?.method==='qr'&&!model.accounts.find(a=>a.id===auth.id)?.connected)readQr(auth);}
  catch(error){showError(error);}finally{model.polling=false;}
 }
 async function open(){if(model.loaded){render();return;}el('accounts-loading').hidden=false;await call('status');el('accounts-loading').hidden=true;}
 function button(label,icon,action,className='button button-secondary'){
  const control=document.createElement('button');control.type='button';control.className=className;control.innerHTML=(icon?getIcon(icon):'')+(className==='icon-button'?'':'<span>'+escapeHtml(label)+'</span>');control.setAttribute('aria-label',label);control.onclick=action;return control;
 }
 function textNode(node,value){if(node.textContent!==value)node.textContent=value;}
 function text(id,value){const node=el(id);if(node.textContent!==value)node.textContent=value;}
 function selectAccount(account){el('account-custom-time').hidden=true;el('account-duration-error').hidden=true;rememberSelection(account.id);model.libraryFor=null;el('account-search').value='';el('account-search-results').replaceChildren();render();if(!account.connected){if(account.active&&!account.input)return;if(account.canResume&&!account.error){call('resume',{id:account.id});return;}openAuth(account);}}
 function confirmRemoval(account){removalTarget=account.id;el('account-remove-warning').textContent=t('accounts_remove_named',{label:account.label});el('account-remove-dialog').showModal();}
 function render(){
  if(!el('accounts-list'))return;
  renderActive();renderAuth();
  const accounts=model.accounts.filter(account=>!account.draft),signature=JSON.stringify([accounts.map(({elapsedSeconds,...account})=>account),model.selected,currentLang,model.busy]);
  if(signature!==model.signature){
   model.signature=signature;const list=document.createDocumentFragment();
   for(const account of accounts){
    const row=document.createElement('div');row.className='account-row'+(account.id===model.selected?' selected':'');row.dataset.account=account.id;
    const select=document.createElement('button');select.type='button';select.className='account-select';select.setAttribute('aria-pressed',String(account.id===model.selected));select.textContent=account.label;select.disabled=model.busy;select.onclick=()=>selectAccount(account);
    const actions=document.createElement('div');actions.className='account-row-actions';
    if(account.connected){const badge=document.createElement('span');badge.className='account-state '+account.state;badge.textContent=t('accounts_state_'+account.state);actions.append(badge);}
    else{const signin=button(t(account.active&&!account.input?'accounts_state_connecting':account.input?'accounts_continue':'accounts_signin'),null,()=>selectAccount(account),'account-state offline account-signin-action');signin.disabled=model.busy;actions.append(signin);}
    const remove=button(t('accounts_remove')+': '+account.label,'x',()=>confirmRemoval(account),'icon-button');remove.classList.add('account-remove-control');remove.dataset.removeAccount=account.id;remove.disabled=model.busy;actions.append(remove);row.append(select,actions);list.append(row);
   }
   el('accounts-list').replaceChildren(list);el('accounts-empty').hidden=accounts.length>0;
  }
  el('accounts-add').disabled=model.busy;el('accounts-retry').disabled=model.busy;
  const account=selected(),ready=!!account?.connected;
  el('account-session-panel').hidden=!ready;el('account-setup').hidden=!ready;el('accounts-workspace').classList.toggle('has-session',ready);
  if(!ready){model.libraryFor=null;model.libraryLoading=false;if(account?.error&&!auth){showError(account.error);}return;}
  text('account-title',account.label);text('account-login',account.login);
  el('account-signout').disabled=model.busy;el('account-presence').value=account.presence||'online';for(const choice of document.querySelectorAll('[data-account-presence]')){choice.disabled=model.busy;choice.setAttribute('aria-pressed',String(choice.dataset.accountPresence===(account.presence||'online')));}
  const running=account.state==='running',editable=!running;
  el('account-start').hidden=running;el('account-stop').hidden=!running;el('account-game-setup').hidden=false;
  el('account-start').disabled=model.busy||!account.games?.length||account.state==='blocked';el('account-stop').disabled=model.busy;
  el('account-refresh-library').disabled=model.busy||!editable;el('account-duration').disabled=model.busy||running;
  const duration=running?account.minutes||0:Number(model.durations[account.id]||0);el('account-duration').value=String(duration);
  for(const preset of document.querySelectorAll('[data-account-duration]')){const value=preset.dataset.accountDuration;const active=value==='custom'?![0,15,30,60].includes(duration):Number(value)===duration;preset.classList.toggle('active',active);preset.setAttribute('aria-pressed',String(active));preset.disabled=model.busy||running;}
  for(const control of el('account-custom-time').querySelectorAll('input,button'))control.disabled=model.busy||running;
  if(running){el('account-custom-time').hidden=true;el('account-duration-error').hidden=true;}
  el('account-search').disabled=model.busy||!editable;el('account-search').closest('.search-field').hidden=running;el('account-library-hint').textContent=t('accounts_library_ready_hint');el('account-library-hint').hidden=running;el('account-quick-games').hidden=!editable||!el('account-quick-picks').querySelector('button');el('account-games-hint').hidden=editable;
  const gameSignature=JSON.stringify([account.id,account.games,model.names,editable,model.busy,currentLang]);
  if(el('account-games').dataset.signature!==gameSignature){
   el('account-games').dataset.signature=gameSignature;const games=document.createDocumentFragment();
   if(running){const heading=document.createElement('div');heading.className='account-games-session';const badge=document.createElement('span');badge.className='account-state running';badge.textContent=t('accounts_state_running');const time=document.createElement('div');time.className='account-game-time';const label=document.createElement('span');label.className='muted';const digits=document.createElement('strong');digits.className='clock-digits';time.append(label,digits);heading.append(badge,time);games.append(heading);}
   for(const id of account.games||[]){const row=document.createElement('div');row.className='account-game-row';row.dataset.game=id;const name=document.createElement('strong');name.textContent=model.names[id]||'Steam App '+id;name.title=name.textContent;const remove=button(t('accounts_remove_game'),'x',()=>call('games',{id:account.id,games:account.games.filter(value=>value!==id)}),'icon-button');remove.setAttribute('aria-label',t('accounts_remove_game')+': '+name.textContent);remove.disabled=model.busy||!editable;remove.hidden=running;row.append(name,remove);games.append(row);}
   el('account-games').replaceChildren(games);el('account-no-games').hidden=!!account.games?.length;el('account-games').hidden=!account.games?.length;
  }
  const gameTime=el('account-games').querySelector('.account-game-time');if(gameTime){const remaining=account.minutes>0;gameTime.querySelector('.muted').textContent=t(remaining?'accounts_remaining':'accounts_elapsed');textNode(gameTime.querySelector('.clock-digits'),clock(remaining?Math.max(0,account.minutes*60-(account.elapsedSeconds||0)):account.elapsedSeconds||0));}
  for(const control of el('account-game-setup').querySelectorAll('#account-search-results button,#account-quick-picks button'))control.disabled=model.busy||!editable;
  if(editable&&model.libraryFor!==account.id&&!model.busy){model.libraryFor=account.id;queueMicrotask(()=>search());}
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
  const running=model.accounts.filter(account=>!account.draft&&account.state==='running');el('accounts-active-panel').hidden=!running.length;text('accounts-session-count',number(running.length));el('accounts-stop-all').disabled=model.busy;window.lootlazyRotations?.refreshSteamSummary?.();
  const signature=JSON.stringify([running.map(account=>[account.id,account.label,account.games,account.minutes]),model.names,model.busy,currentLang]);
  if(list.dataset.signature!==signature){
   list.dataset.signature=signature;const fragment=document.createDocumentFragment();
   for(const account of running){
    const card=document.createElement('section');card.className='account-active-card';card.dataset.account=account.id;
    const heading=document.createElement('div');heading.className='account-active-heading';const title=document.createElement('strong');title.textContent=account.label;
    const badge=document.createElement('span');badge.className='account-state running';badge.textContent=t('accounts_state_running');
    const stop=button(t('quest_stop'),'square',()=>call('stop',{id:account.id}),'button button-danger');stop.disabled=model.busy;const time=document.createElement('div');time.className='account-game-time';const timeLabel=document.createElement('span');timeLabel.className='muted';timeLabel.textContent=t(account.minutes>0?'accounts_remaining':'accounts_elapsed');const digits=document.createElement('strong');digits.className='clock-digits';time.append(timeLabel,digits);heading.append(title,badge,time,stop);card.append(heading);
    for(const id of account.games||[]){const row=document.createElement('div');row.className='account-game-row';const name=document.createElement('strong');name.textContent=model.names[id]||'Steam App '+id;name.title=name.textContent;row.append(name);card.append(row);}
    fragment.append(card);
   }
   list.replaceChildren(fragment);
  }
  for(const account of running){const row=[...list.children].find(item=>item.dataset.account===account.id);if(row)for(const digits of row.querySelectorAll('.clock-digits'))textNode(digits,clock(account.minutes>0?Math.max(0,account.minutes*60-(account.elapsedSeconds||0)):account.elapsedSeconds||0));}
 }

 document.addEventListener('DOMContentLoaded',()=>{
  el('accounts-stop-all').onclick=()=>window.lootlazySteamAccounts.stopAll();
  el('accounts-stop-all').prepend(document.createRange().createContextualFragment(getIcon('square')));
  el('accounts-add').onclick=()=>openAuth();el('accounts-retry').onclick=()=>{model.loaded=false;open();};
  for(const id of ['account-auth-close','account-auth-cancel'])el(id).onclick=()=>closeAuth();el('account-auth-close').innerHTML=getIcon('x');
  el('account-auth-dialog').addEventListener('cancel',event=>{event.preventDefault();closeAuth();});
  el('account-qr-refresh').onclick=el('account-qr-retry').onclick=()=>{if(auth)startQr(auth);};el('account-qr-retry').querySelector('.refresh-icon').innerHTML=getIcon('refresh');el('account-qr-refresh').querySelector('.refresh-icon').innerHTML=getIcon('refresh');
  el('account-auth-form').onsubmit=async event=>{
   event.preventDefault();const context=auth;if(!context||model.busy)return;
   const login=el('account-auth-login').value.trim(),password=el('account-auth-password').value;el('account-auth-password').value='';
   context.method='password';++context.sequence;clearQr();el('account-auth-error').hidden=true;el('account-qr-status').textContent=t('accounts_auth_pending');
   if(!await ensureDraft(context)||!await disconnectAttempt(context))return;
   const account=model.accounts.find(item=>item.id===context.id);
   if(context.draft&&account.login&&account.login!==login){if(!await call('remove',{id:context.id}))return;context.id=null;if(!await ensureDraft(context))return;}
   if(auth===context)await call('signin',{id:context.id,method:'password',login,password});
  };
  el('account-guard-form').onsubmit=async event=>{event.preventDefault();const context=auth;if(!context||model.busy)return;const value=el('account-guard-code').value;el('account-guard-code').value='';el('account-auth-error').hidden=true;await call('input',{id:context.id,challengeId:context.challenge,value});};
  el('account-signout').onclick=()=>{if(selected())call('disconnect',{id:selected().id,signOut:true});};
  for(const choice of document.querySelectorAll('[data-account-presence]'))choice.onclick=()=>{const account=selected();if(account)call('presence',{id:account.id,presence:choice.dataset.accountPresence});};
  function setDuration(minutes){const account=selected();if(!account)return;model.durations[account.id]=minutes;try{localStorage.setItem('lootlazy_account_durations',JSON.stringify(model.durations));}catch{}el('account-custom-time').hidden=true;el('account-duration-error').hidden=true;render();}
  for(const preset of document.querySelectorAll('[data-account-duration]'))preset.onclick=()=>{if(preset.dataset.accountDuration!=='custom'){setDuration(Number(preset.dataset.accountDuration));return;}const minutes=Number(el('account-duration').value)||15;el('account-hours').value=String(Math.floor(minutes/60));el('account-minutes').value=String(minutes%60);el('account-custom-time').hidden=false;el('account-duration-error').hidden=true;el('account-hours').focus();};
  el('account-time-save').onclick=()=>{const hours=Number(el('account-hours').value),mins=Number(el('account-minutes').value),minutes=hours*60+mins;if(!el('account-hours').value||!el('account-minutes').value||!Number.isInteger(hours)||!Number.isInteger(mins)||hours<0||mins<0||mins>59||minutes<1||minutes>1440){el('account-duration-error').hidden=false;return;}setDuration(minutes);};
  el('account-time-cancel').onclick=()=>{el('account-custom-time').hidden=true;el('account-duration-error').hidden=true;};
  el('account-start').onclick=()=>{if(selected())call('start',{id:selected().id,minutes:Number(el('account-duration').value)});};
  el('account-stop').onclick=()=>{if(selected())call('stop',{id:selected().id});};
  el('account-remove-cancel').onclick=()=>{removalTarget=null;el('account-remove-dialog').close();};
  el('account-remove-confirm').onclick=async()=>{const id=removalTarget;removalTarget=null;el('account-remove-dialog').close();if(id)await call('remove',{id});};
  el('account-search').oninput=()=>{clearTimeout(model.searchTimer);++model.searchSequence;model.searchTimer=setTimeout(()=>search(),300);};
  for(const [id,icon]of [['accounts-add','plus'],['accounts-retry','refresh'],['account-start','play'],['account-stop','square']]){const span=document.createElement('span');span.innerHTML=getIcon(icon);el(id).prepend(span);}
  el('account-refresh-library').onclick=()=>search(true);el('account-refresh-library').innerHTML=getIcon('refresh');el('account-search-icon').innerHTML=getIcon('search');el('steam-mode-accounts-icon').innerHTML=getIcon('users');
  setInterval(poll,1500);
 });
 window.lootlazySteamAccounts={open,translate(){model.signature='';render();},async stopAll(){for(const account of [...model.accounts])if(account.state==='running'&&!await call('stop',{id:account.id}))return false;return true;},activeCount:()=>model.accounts.filter(account=>!account.draft&&account.state==='running').length};
})();
