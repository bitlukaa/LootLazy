(() => {
  const el = id => document.getElementById(id);
  let lastDiscord = 'simulator';
  function clientStatus(){const discord=['simulator','variety','quests'].includes(state.tab),steam=state.tab==='steam',service=state.steamService||'idle';el('discord-indicator').hidden=!discord;el('steam-indicator').hidden=!steam||service==='accounts';el('steam-account-summary').hidden=!steam||service!=='accounts';el('service-client-status').hidden=!discord&&!steam;el('steam-idle-nav').hidden=!steam||service!=='idle';el('accounts-sub-nav').hidden=!steam||service!=='accounts';document.querySelectorAll('[data-steam-service]').forEach(button=>button.classList.toggle('active',steam&&button.dataset.steamService===service));if(steam){const title=el('view-steam').querySelector('h1');title.removeAttribute('data-i18n');title.textContent=t('steam_service_'+service);}}
  function navigation(tab) {
    const discord = ["simulator", "variety", "quests"].includes(tab),
      steam = tab === "steam";
    if (discord) lastDiscord = tab;
    el("nav-discord").classList.toggle("active", discord);
    el("discord-service-nav").hidden = !discord;
    el("steam-service-nav").hidden = !steam;
    document.querySelector(".service-bar").hidden = !discord && !steam;
    clientStatus();
  }
  document.addEventListener('DOMContentLoaded', () => {
    el('nav-discord').onclick = () => switchTab(lastDiscord);
    el('header-donate-icon').innerHTML=getIcon('heart');el('support-heart').innerHTML=getIcon('heart');
    for(const [id,icon]of [['service-idle-icon','monitor'],['service-accounts-icon','users'],['service-achievements-icon','trophy']])el(id).innerHTML=getIcon(icon);
    for(const button of document.querySelectorAll('[data-steam-service]'))button.onclick=()=>setSteamSubMode(button.dataset.steamService==='idle'?(state.lastIdleMode||'library'):button.dataset.steamService==='accounts'?(state.lastAccountMode||'accounts'):'achievements');
    navigation(state.tab);
  });
  window.lootlazyPlatform = { navigation, clientStatus, translate() { navigation(state.tab); } };
})();
