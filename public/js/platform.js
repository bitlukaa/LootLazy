(() => {
  const el = (id) => document.getElementById(id);
  let unlock;
  window.lootlazyIdentityReady = new Promise((resolve) => {
    unlock = resolve;
  });
  const model = {
    identity: null,
    providers: [],
    checking: false,
    lastDiscord: "simulator",
    pending: false,
    started: false,
    configLoaded: false,
  };
  const raw = (route, body) => window.lootlazyRequest(route, body);
  function message(error) {
    const value = String(error?.message || error);
    return t(
      [
        "platform_auth_unconfigured",
        "platform_auth_unavailable",
        "platform_auth_error",
        "platform_auth_busy",
        "platform_auth_storage",
        "platform_auth_timeout",
        "platform_auth_cancelled",
        "platform_auth_required",
      ].find((key) => value.includes(key)) || "platform_auth_error",
    );
  }
  function label(id, value) {
    if (el(id).textContent !== value) el(id).textContent = value;
  }
  function status(value) {
    for (const id of ["platform-gate-status", "platform-dialog-status"])
      label(id, value);
  }
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
    if (discord) model.lastDiscord = tab;
    el("nav-discord").classList.toggle("active", discord);
    el("discord-service-nav").hidden = !discord;
    el("steam-service-nav").hidden = !steam;
    clientStatus();
  }
  function providers() {
    for (const id of ["platform-gate-providers", "platform-dialog-providers"]) {
      const list = el(id);
      list.replaceChildren();
      for (const provider of ["google", "discord", "steam"]) {
        const button = document.createElement("button");
        button.type = "button";
        button.className = "button button-secondary";
        button.dataset.platformProvider = provider;
        button.textContent = t("platform_login_" + provider);
        button.disabled = model.pending || !model.providers.includes(provider);
        button.onclick = () => begin(provider);
        list.append(button);
      }
    }
  }
  function render() {
    const identity = model.identity;
    label(
      "platform-account-label",
      identity?.authenticated
        ? identity.user?.name || "LootLazy"
        : t("platform_signin"),
    );
    el("platform-account-label").removeAttribute("data-i18n");
    el("platform-user-summary").hidden = !identity?.authenticated;
    el("platform-dialog-providers").hidden = !!identity?.authenticated;
    label("platform-user-name", identity?.user?.name || "");
    el("platform-gate-retry").hidden = model.pending;
    providers();
  }
  async function check() {
    if (model.checking) return;
    model.checking = true;
    try {
      const identity = await raw("/api/identity/status");
      if (typeof identity?.enforced !== "boolean")
        throw Error("platform_auth_error");
      model.identity = identity;
      model.pending = !!identity.pending;
      const locked = identity.enforced && !identity.authenticated;
      el("platform-login-gate").hidden = !locked;
      document.querySelector(".app-shell").hidden = locked;
      if (!identity.enforced || identity.authenticated) unlock();
      if (identity.configured && !model.configLoaded) {
        try {
          const config = await raw("/api/identity/config");
          model.providers = config.providers || [];
          model.configLoaded = true;
        } catch {
          model.providers = [];
        }
      } else if (!identity.configured) model.providers = [];
      status(
        identity.pending
          ? t("platform_auth_wait")
          : identity.error
            ? message(identity.error)
            : identity.configured
              ? ""
              : t(
                  identity.enforced
                    ? "platform_auth_unconfigured"
                    : "platform_auth_preview",
                ),
      );
      render();
      if (model.started && identity.authenticated) {
        location.reload();
      }
    } catch (error) {
      status(message(error));
      if (!model.identity) {
        el("platform-login-gate").hidden = false;
        document.querySelector(".app-shell").hidden = true;
      }
    } finally {
      model.checking = false;
    }
  }
  async function begin(provider) {
    if (model.pending) return;
    model.pending = true;
    model.started = true;
    status(t("platform_auth_wait"));
    providers();
    try {
      await raw("/api/identity/begin", { provider });
      await check();
    } catch (error) {
      model.pending = false;
      model.started = false;
      status(message(error));
      providers();
    }
  }
  function expired() {
    if (model.identity?.enforced) location.reload();
  }
  document.addEventListener("DOMContentLoaded", () => {
    el("nav-discord").onclick = () => switchTab(model.lastDiscord);
    el("platform-account").querySelector(".platform-user-icon").innerHTML =
      getIcon("users");
    el("platform-plans").onclick = () =>
      el("platform-plans-dialog").showModal();
    el("platform-plans-close").innerHTML = getIcon("x");
    el("platform-plans-close").onclick = () =>
      el("platform-plans-dialog").close();
    el("platform-account").onclick = () => {
      render();
      el("platform-login-dialog").showModal();
    };
    el("platform-login-close").innerHTML = getIcon("x");
    el("platform-login-close").onclick = () =>
      el("platform-login-dialog").close();
    el("platform-login-dialog").addEventListener("close", async () => {
      if (model.pending) {
        await raw("/api/identity/cancel", {});
        model.started = false;
        await check();
      }
    });
    el("platform-signout").onclick = async () => {
      try {
        await raw("/api/identity/signout", {});
        location.reload();
      } catch (error) {
        status(message(error));
      }
    };
    el("platform-gate-retry").onclick = () => {
      model.configLoaded = false;
      check();
    };
    navigation(state.tab);
    check();
    setInterval(() => {
      if (model.pending) check();
    }, 1000);
  });
  window.lootlazyPlatform = {
    navigation,
    clientStatus,
    expired,
    translate() {
      navigation(state.tab);
      render();
    },
  };
})();
