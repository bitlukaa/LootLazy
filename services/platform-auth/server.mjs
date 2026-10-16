import { randomBytes, createHash, timingSafeEqual } from "node:crypto";
import { DatabaseSync } from "node:sqlite";
import { mkdirSync, chmodSync } from "node:fs";
import { dirname } from "node:path";
import { createServer } from "node:http";
import { pathToFileURL } from "node:url";
const token = () => randomBytes(32).toString("base64url"),
  hash = (value) => createHash("sha256").update(value).digest("base64url");
const valid = (value) =>
  typeof value === "string" && /^[A-Za-z0-9_-]{43}$/.test(value);
const equal = (a, b) =>
  typeof a === "string" &&
  typeof b === "string" &&
  a.length === b.length &&
  timingSafeEqual(Buffer.from(a), Buffer.from(b));
const fail = (status, message) => {
  throw Object.assign(new Error(message), { status });
};
const escape = (value) =>
  String(value).replace(
    /[&<>"']/g,
    (c) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
        c
      ],
  );
export function callbackUrl(value) {
  const u = new URL(value);
  if (
    u.protocol !== "http:" ||
    u.hostname !== "127.0.0.1" ||
    !u.port ||
    Number(u.port) < 1024 ||
    u.username ||
    u.password ||
    u.search ||
    u.hash ||
    !/^\/lootlazy\/callback\/[a-zA-Z0-9_-]{43}$/.test(u.pathname)
  )
    fail(400, "Invalid desktop callback");
  return u.href;
}
export function createService(
  config,
  { fetcher = fetch, clock = () => Date.now() } = {},
) {
  const origin = new URL(config.publicUrl);
  if (
    origin.protocol !== "https:" &&
    !(
      config.allowLocal &&
      origin.protocol === "http:" &&
      origin.hostname === "127.0.0.1"
    )
  )
    throw Error("PUBLIC_URL requires HTTPS");
  if (
    origin.pathname !== "/" ||
    origin.search ||
    origin.hash ||
    origin.username ||
    origin.password
  )
    throw Error("PUBLIC_URL must be an origin");
  if (config.database !== ":memory:") {
    mkdirSync(dirname(config.database), { recursive: true, mode: 0o700 });
  }
  const db = new DatabaseSync(config.database);
  if (config.database !== ":memory:") chmodSync(config.database, 0o600);
  db.exec(`PRAGMA journal_mode=WAL;PRAGMA foreign_keys=ON;
 CREATE TABLE IF NOT EXISTS users(id TEXT PRIMARY KEY,name TEXT NOT NULL,created INTEGER NOT NULL);
 CREATE TABLE IF NOT EXISTS identities(provider TEXT NOT NULL,subject TEXT NOT NULL,user_id TEXT NOT NULL REFERENCES users(id),PRIMARY KEY(provider,subject));
 CREATE TABLE IF NOT EXISTS attempts(id TEXT PRIMARY KEY,provider TEXT NOT NULL,callback TEXT NOT NULL,challenge TEXT NOT NULL,client_state TEXT NOT NULL,oauth_state TEXT UNIQUE,browser_hash TEXT,expires INTEGER NOT NULL);
 CREATE TABLE IF NOT EXISTS grants(id TEXT PRIMARY KEY,user_id TEXT NOT NULL,challenge TEXT NOT NULL,expires INTEGER NOT NULL);
 CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY,user_id TEXT NOT NULL,access_hash TEXT UNIQUE NOT NULL,refresh_hash TEXT UNIQUE NOT NULL,access_until INTEGER NOT NULL,refresh_until INTEGER NOT NULL,revoked INTEGER NOT NULL DEFAULT 0);
 CREATE TABLE IF NOT EXISTS used_refresh(hash TEXT PRIMARY KEY,session_id TEXT NOT NULL,expires INTEGER NOT NULL);
 CREATE TABLE IF NOT EXISTS nonces(value TEXT PRIMARY KEY,expires INTEGER NOT NULL);`);
  const providers = {
    google:
      config.googleId && config.googleSecret
        ? {
            authorize: "https://accounts.google.com/o/oauth2/v2/auth",
            token: "https://oauth2.googleapis.com/token",
            userinfo: "https://openidconnect.googleapis.com/v1/userinfo",
            id: config.googleId,
            secret: config.googleSecret,
            scope: "openid profile",
          }
        : null,
    discord:
      config.discordId && config.discordSecret
        ? {
            authorize: "https://discord.com/oauth2/authorize",
            token: "https://discord.com/api/oauth2/token",
            userinfo: "https://discord.com/api/users/@me",
            id: config.discordId,
            secret: config.discordSecret,
            scope: "identify",
          }
        : null,
    steam: config.steam === false ? null : {},
  };
  const limits = new Map();
  function rate(key, max = 60) {
    const now = clock(),
      entry = limits.get(key);
    if (!entry || entry.until < now) {
      if (limits.size >= 10000) limits.clear();
      limits.set(key, { until: now + 60000, count: 1 });
    } else if (++entry.count > max) fail(429, "Please wait and try again");
  }
  const run = (sql, ...args) => db.prepare(sql).run(...args),
    get = (sql, ...args) => db.prepare(sql).get(...args);
  function transaction(fn) {
    db.exec("BEGIN IMMEDIATE");
    try {
      const result = fn();
      db.exec("COMMIT");
      return result;
    } catch (error) {
      db.exec("ROLLBACK");
      throw error;
    }
  }
  function prune() {
    const now = clock();
    for (const table of ["attempts", "grants", "used_refresh", "nonces"])
      run(`DELETE FROM ${table} WHERE expires < ?`, now);
    run("DELETE FROM sessions WHERE refresh_until < ?", now);
  }
  function identify(provider, subject, name) {
    if (!subject || typeof subject !== "string" || subject.length > 128)
      fail(401, "Provider identity is invalid");
    return transaction(() => {
      const existing = get(
        "SELECT user_id FROM identities WHERE provider=? AND subject=?",
        provider,
        subject,
      );
      if (existing) return existing.user_id;
      const id = token();
      run(
        "INSERT INTO users VALUES(?,?,?)",
        id,
        String(name || "LootLazy user").slice(0, 80),
        clock(),
      );
      run("INSERT INTO identities VALUES(?,?,?)", provider, subject, id);
      return id;
    });
  }
  function issue(userId, sessionId = null) {
    const access = token(),
      refresh = token(),
      now = clock(),
      id = sessionId || token();
    if (sessionId)
      run(
        "UPDATE sessions SET access_hash=?,refresh_hash=?,access_until=?,refresh_until=? WHERE id=?",
        hash(access),
        hash(refresh),
        now + 900000,
        now + 2592000000,
        id,
      );
    else
      run(
        "INSERT INTO sessions VALUES(?,?,?,?,?,?,0)",
        id,
        userId,
        hash(access),
        hash(refresh),
        now + 900000,
        now + 2592000000,
      );
    return {
      accessToken: access,
      refreshToken: refresh,
      expiresIn: 900,
      user: {
        id: userId,
        name: get("SELECT name FROM users WHERE id=?", userId).name,
      },
      plan: "free",
    };
  }
  async function remote(url, options = {}) {
    const response = await fetcher(url, {
      ...options,
      redirect: "error",
      signal: AbortSignal.timeout(15000),
    });
    if (!response.ok)
      fail(502, "Identity provider could not complete the request");
    const text = await response.text();
    if (text.length > 262144)
      fail(502, "Identity provider response is too large");
    return text;
  }
  async function jsonRemote(url, options) {
    return JSON.parse(await remote(url, options));
  }
  function bearer(headers) {
    const value = headers.authorization || "";
    if (!/^Bearer [A-Za-z0-9_-]{43}$/.test(value))
      fail(401, "Sign in required");
    const session = get(
      "SELECT * FROM sessions WHERE access_hash=?",
      hash(value.slice(7)),
    );
    if (!session || session.revoked || session.access_until < clock())
      fail(401, "Sign in required");
    return session;
  }
  function cookie(headers, state) {
    const cookies = String(headers.cookie || "")
      .split(";")
      .map((v) => v.trim());
    return (
      cookies
        .find((v) => v.startsWith("ll_" + state + "="))
        ?.split("=")
        .slice(1)
        .join("=") || ""
    );
  }
  async function dispatch(
    method,
    path,
    { body = {}, headers = {}, ip = "local" } = {},
  ) {
    const u = new URL(path, origin);
    prune();
    rate(ip);
    const route = u.pathname;
    if (method === "GET" && route === "/health")
      return { status: 200, json: { ok: true } };
    if (method === "GET" && route === "/v1/config")
      return {
        status: 200,
        json: {
          providers: Object.keys(providers).filter((p) => providers[p]),
          plans: [{ id: "free", name: "Free" }],
        },
      };
    if (method === "POST" && route === "/v1/login/start") {
      rate("start:" + ip, 15);
      if (
        !providers[body.provider] ||
        !valid(body.challenge) ||
        !valid(body.state)
      )
        fail(400, "Invalid sign-in request");
      const callback = callbackUrl(body.callback),
        ticket = token();
      run(
        "INSERT INTO attempts VALUES(?,?,?,?,?,NULL,NULL,?)",
        hash(ticket),
        body.provider,
        callback,
        body.challenge,
        body.state,
        clock() + 300000,
      );
      return {
        status: 200,
        json: {
          authorizeUrl: origin.origin + "/v1/authorize?ticket=" + ticket,
        },
      };
    }
    if (method === "GET" && route === "/v1/authorize") {
      const ticket = u.searchParams.get("ticket");
      if (!valid(ticket)) fail(400, "Invalid sign-in attempt");
      const attempt = get("SELECT * FROM attempts WHERE id=?", hash(ticket));
      if (!attempt || attempt.expires < clock() || attempt.oauth_state)
        fail(400, "Sign-in attempt expired");
      const state = token(),
        browser = token();
      run(
        "UPDATE attempts SET oauth_state=?,browser_hash=? WHERE id=?",
        state,
        hash(browser),
        attempt.id,
      );
      const callback = origin.origin + "/v1/callback/" + attempt.provider;
      const target = new URL(
        attempt.provider === "steam"
          ? "https://steamcommunity.com/openid/login"
          : providers[attempt.provider].authorize,
      );
      if (attempt.provider === "steam") {
        callback;
        const ret = new URL(callback);
        ret.searchParams.set("state", state);
        for (const [key, value] of Object.entries({
          "openid.ns": "http://specs.openid.net/auth/2.0",
          "openid.mode": "checkid_setup",
          "openid.return_to": ret.href,
          "openid.realm": origin.origin,
          "openid.identity":
            "http://specs.openid.net/auth/2.0/identifier_select",
          "openid.claimed_id":
            "http://specs.openid.net/auth/2.0/identifier_select",
        }))
          target.searchParams.set(key, value);
      } else {
        const provider = providers[attempt.provider];
        for (const [key, value] of Object.entries({
          client_id: provider.id,
          redirect_uri: callback,
          response_type: "code",
          scope: provider.scope,
          state,
        }))
          target.searchParams.set(key, value);
      }
      return {
        status: 302,
        headers: {
          Location: target.href,
          "Set-Cookie": `ll_${state}=${browser}; HttpOnly; SameSite=Lax; Path=/v1/callback; Max-Age=300${origin.protocol === "https:" ? "; Secure" : ""}`,
        },
      };
    }
    if (method === "GET" && route.startsWith("/v1/callback/")) {
      const providerName = route.slice("/v1/callback/".length),
        state = u.searchParams.get("state");
      if (!providers[providerName] || !valid(state))
        fail(400, "Invalid callback");
      const attempt = get(
        "SELECT * FROM attempts WHERE oauth_state=? AND provider=?",
        state,
        providerName,
      );
      if (
        !attempt ||
        attempt.expires < clock() ||
        !equal(attempt.browser_hash, hash(cookie(headers, state)))
      )
        fail(400, "Sign-in attempt expired");
      // Consume before awaiting a provider: callbacks cannot be replayed concurrently.
      run("DELETE FROM attempts WHERE id=?", attempt.id);
      if (
        u.searchParams.has("error") ||
        u.searchParams.get("openid.mode") === "cancel"
      )
        return {
          status: 302,
          headers: {
            Location:
              attempt.callback +
              "?state=" +
              attempt.client_state +
              "&error=cancelled",
          },
        };
      let subject, name;
      if (providerName === "steam") {
        const claimed = u.searchParams.get("openid.claimed_id") || "",
          identity = u.searchParams.get("openid.identity"),
          nonce = u.searchParams.get("openid.response_nonce") || "",
          signed = new Set(
            (u.searchParams.get("openid.signed") || "").split(","),
          );
        const expected = new URL(origin.origin + "/v1/callback/steam");
        expected.searchParams.set("state", state);
        if (
          u.searchParams.get("openid.ns") !==
            "http://specs.openid.net/auth/2.0" ||
          u.searchParams.get("openid.mode") !== "id_res" ||
          u.searchParams.get("openid.op_endpoint") !==
            "https://steamcommunity.com/openid/login" ||
          claimed !== identity ||
          !/^https:\/\/steamcommunity\.com\/openid\/id\/\d{17}$/.test(
            claimed,
          ) ||
          u.searchParams.get("openid.return_to") !== expected.href ||
          ![
            "op_endpoint",
            "claimed_id",
            "identity",
            "return_to",
            "response_nonce",
            "assoc_handle",
          ].every((field) => signed.has(field)) ||
          !Number.isFinite(Date.parse(nonce.slice(0, 20))) ||
          Math.abs(clock() - Date.parse(nonce.slice(0, 20))) > 300000 ||
          get("SELECT value FROM nonces WHERE value=?", nonce)
        )
          fail(401, "Steam identity is invalid");
        const data = new URLSearchParams(
          [...u.searchParams].filter(([key]) => key.startsWith("openid.")),
        );
        data.set("openid.mode", "check_authentication");
        const checked = await remote(
          "https://steamcommunity.com/openid/login",
          {
            method: "POST",
            headers: { "Content-Type": "application/x-www-form-urlencoded" },
            body: data,
          },
        );
        if (!/^is_valid:true\r?$/m.test(checked))
          fail(401, "Steam identity could not be verified");
        run("INSERT INTO nonces VALUES(?,?)", nonce, clock() + 600000);
        subject = claimed.split("/").at(-1);
        name = "Steam user";
      } else {
        const provider = providers[providerName],
          code = u.searchParams.get("code");
        if (!code || code.length > 4096)
          fail(401, "Provider authorization is missing");
        const data = new URLSearchParams({
          client_id: provider.id,
          client_secret: provider.secret,
          code,
          grant_type: "authorization_code",
          redirect_uri: origin.origin + "/v1/callback/" + providerName,
        });
        const tokens = await jsonRemote(provider.token, {
          method: "POST",
          headers: { "Content-Type": "application/x-www-form-urlencoded" },
          body: data,
        });
        if (
          typeof tokens.access_token !== "string" ||
          tokens.access_token.length > 8192
        )
          fail(401, "Provider authorization is invalid");
        const user = await jsonRemote(provider.userinfo, {
          headers: { Authorization: "Bearer " + tokens.access_token },
        });
        subject = providerName === "google" ? user.sub : user.id;
        name =
          providerName === "google"
            ? user.name
            : user.global_name || user.username;
      }
      const userId = identify(providerName, subject, name),
        code = token();
      run(
        "INSERT INTO grants VALUES(?,?,?,?)",
        hash(code),
        userId,
        attempt.challenge,
        clock() + 60000,
      );
      return {
        status: 302,
        headers: {
          Location:
            attempt.callback +
            "?code=" +
            code +
            "&state=" +
            attempt.client_state,
          "Set-Cookie": `ll_${state}=; HttpOnly; SameSite=Lax; Path=/v1/callback; Max-Age=0${origin.protocol === "https:" ? "; Secure" : ""}`,
        },
      };
    }
    if (method === "POST" && route === "/v1/login/exchange") {
      if (!valid(body.code) || !valid(body.verifier))
        fail(400, "Invalid authorization code");
      return transaction(() => {
        const grant = get("SELECT * FROM grants WHERE id=?", hash(body.code));
        if (
          !grant ||
          grant.expires < clock() ||
          !equal(grant.challenge, hash(body.verifier))
        )
          fail(401, "Authorization code expired");
        run("DELETE FROM grants WHERE id=?", grant.id);
        return { status: 200, json: issue(grant.user_id) };
      });
    }
    if (method === "POST" && route === "/v1/session/refresh") {
      if (!valid(body.refreshToken)) fail(401, "Sign in required");
      const hashed = hash(body.refreshToken),
        old = get("SELECT * FROM used_refresh WHERE hash=?", hashed);
      if (old) {
        run("UPDATE sessions SET revoked=1 WHERE id=?", old.session_id);
        fail(401, "Sign in required");
      }
      return transaction(() => {
        const session = get(
          "SELECT * FROM sessions WHERE refresh_hash=?",
          hashed,
        );
        if (!session || session.revoked || session.refresh_until < clock())
          fail(401, "Sign in required");
        run(
          "INSERT INTO used_refresh VALUES(?,?,?)",
          hashed,
          session.id,
          session.refresh_until,
        );
        return { status: 200, json: issue(session.user_id, session.id) };
      });
    }
    if (method === "GET" && route === "/v1/session") {
      const session = bearer(headers);
      return {
        status: 200,
        json: {
          user: {
            id: session.user_id,
            name: get("SELECT name FROM users WHERE id=?", session.user_id)
              .name,
          },
          plan: "free",
        },
      };
    }
    if (method === "POST" && route === "/v1/session/logout") {
      const session = valid(body.refreshToken)
        ? get(
            "SELECT * FROM sessions WHERE refresh_hash=? AND revoked=0 AND refresh_until>?",
            hash(body.refreshToken),
            clock(),
          )
        : bearer(headers);
      if (!session) fail(401, "Sign in required");
      run("UPDATE sessions SET revoked=1 WHERE id=?", session.id);
      return { status: 200, json: { ok: true } };
    }
    fail(404, "Not found");
  }
  async function safe(...args) {
    try {
      return await dispatch(...args);
    } catch (error) {
      return {
        status: error.status || 500,
        json: {
          error: error.status ? error.message : "Account service unavailable",
        },
      };
    }
  }
  const server = createServer(async (req, res) => {
    res.setHeader("Cache-Control", "no-store");
    res.setHeader("Referrer-Policy", "no-referrer");
    res.setHeader("X-Content-Type-Options", "nosniff");
    res.setHeader(
      "Content-Security-Policy",
      "default-src 'none'; frame-ancestors 'none'; base-uri 'none'",
    );
    let size = 0,
      raw = "";
    try {
      for await (const bytes of req) {
        size += bytes.length;
        if (size > 16384) fail(413, "Request too large");
        raw += bytes;
      }
      const result = await safe(req.method, req.url, {
        body: raw ? JSON.parse(raw) : {},
        headers: req.headers,
        ip: req.socket.remoteAddress || "unknown",
      });
      res.writeHead(result.status, {
        ...result.headers,
        "Content-Type": result.json
          ? "application/json"
          : "text/html; charset=utf-8",
      });
      res.end(
        result.json
          ? JSON.stringify(result.json)
          : "<!doctype html><title>LootLazy</title><p>Continue in LootLazy.</p>",
      );
    } catch {
      res.writeHead(400, { "Content-Type": "application/json" });
      res.end('{"error":"Invalid request"}');
    }
  });
  server.requestTimeout = 15000;
  server.headersTimeout = 10000;
  return {
    server,
    dispatch: safe,
    close() {
      server.close();
      db.close();
    },
  };
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  process.umask(0o077);
  const service = createService({
    publicUrl: process.env.PUBLIC_URL,
    database: process.env.DATABASE_PATH || "./data/accounts.sqlite",
    googleId: process.env.GOOGLE_CLIENT_ID,
    googleSecret: process.env.GOOGLE_CLIENT_SECRET,
    discordId: process.env.DISCORD_CLIENT_ID,
    discordSecret: process.env.DISCORD_CLIENT_SECRET,
  });
  service.server.listen(
    Number(process.env.PORT || 8787),
    process.env.HOST || "127.0.0.1",
    () => console.log("LootLazy account service ready"),
  );
}
