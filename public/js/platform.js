(() => {
  const el = id => document.getElementById(id);
  let lastDiscord = 'simulator';
  function clientStatus() {
    const discord = ["simulator", "variety", "quests"].includes(state.tab),
      steam = state.tab === "steam",
      accounts = steam && state.steamSubMode === "accounts";
    el("discord-indicator").hidden = !discord;
    el("steam-indicator").hidden = !steam || state.steamSubMode !== "library";
    if (el("steam-account-summary"))
      el("steam-account-summary").hidden = !accounts;
    el("service-client-status").hidden = !discord && !steam;
    if (steam) {
      const title = el("view-steam").querySelector("h1");
      title.removeAttribute("data-i18n");
      title.textContent = t(
        state.steamSubMode === "accounts"
          ? "accounts_title"
          : state.steamSubMode === "active"
            ? "steam_mode_active"
            : "steam_mode_library",
      );
    }
  }
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
    el('header-donate-icon').innerHTML = getIcon('heart');
    navigation(state.tab);
  });
  window.lootlazyPlatform = { navigation, clientStatus, translate() { navigation(state.tab); } };
})();
