import test from "node:test";
import assert from "node:assert/strict";
import { randomBytes, createHash } from "node:crypto";
import { createService, callbackUrl } from "./server.mjs";
const token = () => randomBytes(32).toString("base64url"),
  hash = (value) => createHash("sha256").update(value).digest("base64url");
function fixture() {
  let now = Date.now(),
    invalidSteam = false;
  const service = createService(
    {
      publicUrl: "https://accounts.example",
      database: ":memory:",
      googleId: "fixture-google",
      googleSecret: "fixture-secret",
      discordId: "fixture-discord",
      discordSecret: "fixture-secret",
    },
    {
      clock: () => now,
      fetcher: async (url) => ({
        ok: true,
        text: async () =>
          String(url).includes("openid/login")
            ? invalidSteam
              ? "is_valid:false\n"
              : "ns:http://specs.openid.net/auth/2.0\nis_valid:true\n"
            : JSON.stringify(
                String(url).includes("token")
                  ? { access_token: "fixture-provider-token" }
                  : String(url).includes("google")
                    ? { sub: "google-user", name: "Test user" }
                    : { id: "discord-user", username: "Discord user" },
              ),
      }),
    },
  );
  return {
    ...service,
    advance(seconds) {
      now += seconds * 1000;
    },
    invalidSteam() {
      invalidSteam = true;
    },
    now: () => now,
  };
}
async function begin(service, provider = "google") {
  const verifier = token(),
    state = token(),
    callback = "http://127.0.0.1:32999/lootlazy/callback/" + token();
  const started = await service.dispatch("POST", "/v1/login/start", {
    body: { provider, challenge: hash(verifier), state, callback },
  });
  assert.equal(started.status, 200);
  const browser = await service.dispatch("GET", started.json.authorizeUrl);
  assert.equal(browser.status, 302);
  const auth = new URL(browser.headers.Location);
  const oauthState =
    auth.searchParams.get("state") ||
    new URL(auth.searchParams.get("openid.return_to")).searchParams.get(
      "state",
    );
  return {
    verifier,
    state,
    callback,
    oauthState,
    cookie: browser.headers["Set-Cookie"].split(";")[0],
  };
}
async function approve(service, context, provider = "google", extra = {}) {
  const url = new URL("https://accounts.example/v1/callback/" + provider);
  url.searchParams.set("state", context.oauthState);
  if (provider === "steam") {
    for (const [key, value] of Object.entries({
      "openid.ns": "http://specs.openid.net/auth/2.0",
      "openid.mode": "id_res",
      "openid.op_endpoint": "https://steamcommunity.com/openid/login",
      "openid.claimed_id":
        "https://steamcommunity.com/openid/id/76561198000000000",
      "openid.identity":
        "https://steamcommunity.com/openid/id/76561198000000000",
      "openid.return_to":
        "https://accounts.example/v1/callback/steam?state=" +
        context.oauthState,
      "openid.response_nonce":
        new Date(service.now())
          .toISOString()
          .replace(".000Z", "Z")
          .slice(0, 19) +
        "Z" +
        token(),
      "openid.assoc_handle": "fixture",
      "openid.signed":
        "op_endpoint,claimed_id,identity,return_to,response_nonce,assoc_handle",
    }))
      url.searchParams.set(key, value);
  } else url.searchParams.set("code", "fixture-code");
  for (const [key, value] of Object.entries(extra))
    url.searchParams.set(key, value);
  return service.dispatch("GET", url.href, {
    headers: { cookie: context.cookie },
  });
}
async function login(service, provider = "google") {
  const context = await begin(service, provider);
  const callback = await approve(service, context, provider);
  assert.equal(callback.status, 302, JSON.stringify(callback));
  const target = new URL(callback.headers.Location);
  assert.equal(target.searchParams.get("state"), context.state);
  const grant = target.searchParams.get("code");
  const exchange = await service.dispatch("POST", "/v1/login/exchange", {
    body: { code: grant, verifier: context.verifier },
  });
  assert.equal(exchange.status, 200);
  return exchange.json;
}
test("callback destinations are restricted to the initiating desktop loopback route", () => {
  for (const callback of [
    "https://evil.example/",
    "http://localhost:32999/lootlazy/callback/" + token(),
    "http://127.0.0.1:80/lootlazy/callback/" + token(),
    "http://127.0.0.1:32999/lootlazy/callback/" + token() + "?x=y",
  ])
    assert.throws(() => callbackUrl(callback));
  assert.doesNotThrow(() =>
    callbackUrl("http://127.0.0.1:32999/lootlazy/callback/" + token()),
  );
});
test("Google and Discord authenticate through verified provider responses and keep distinct identities", async () => {
  const f = fixture();
  try {
    const google = await login(f),
      again = await login(f),
      discord = await login(f, "discord");
    assert.equal(google.user.id, again.user.id);
    assert.notEqual(google.user.id, discord.user.id);
    const profile = await f.dispatch("GET", "/v1/session", {
      headers: { authorization: "Bearer " + google.accessToken },
    });
    assert.equal(profile.status, 200);
    assert.equal(profile.json.plan, "free");
    assert.equal(profile.json.accessToken, undefined);
  } finally {
    f.close();
  }
});
test("callback cookie, one-time state and PKCE prevent replay and code interception", async () => {
  const f = fixture();
  try {
    const ctx = await begin(f);
    const wrong = await f.dispatch(
      "GET",
      "/v1/callback/google?state=" + ctx.oauthState + "&code=x",
      { headers: { cookie: "ll_" + ctx.oauthState + "=" + token() } },
    );
    assert.equal(wrong.status, 400);
    const result = await approve(f, ctx);
    assert.equal(result.status, 302);
    assert.equal((await approve(f, ctx)).status, 400);
    const code = new URL(result.headers.Location).searchParams.get("code");
    assert.equal(
      (
        await f.dispatch("POST", "/v1/login/exchange", {
          body: { code, verifier: token() },
        })
      ).status,
      401,
    );
    assert.equal(
      (
        await f.dispatch("POST", "/v1/login/exchange", {
          body: { code, verifier: ctx.verifier },
        })
      ).status,
      200,
    );
    assert.equal(
      (
        await f.dispatch("POST", "/v1/login/exchange", {
          body: { code, verifier: ctx.verifier },
        })
      ).status,
      401,
    );
  } finally {
    f.close();
  }
});
test("expired login attempts and grants cannot establish a session", async () => {
  const f = fixture();
  try {
    const ctx = await begin(f);
    f.advance(301);
    assert.equal((await approve(f, ctx)).status, 400);
    const next = await begin(f);
    const result = await approve(f, next);
    const code = new URL(result.headers.Location).searchParams.get("code");
    f.advance(61);
    assert.equal(
      (
        await f.dispatch("POST", "/v1/login/exchange", {
          body: { code, verifier: next.verifier },
        })
      ).status,
      401,
    );
  } finally {
    f.close();
  }
});
test("refresh rotation rejects reuse and revokes the affected token family", async () => {
  const f = fixture();
  try {
    const session = await login(f);
    f.advance(901);
    assert.equal(
      (
        await f.dispatch("GET", "/v1/session", {
          headers: { authorization: "Bearer " + session.accessToken },
        })
      ).status,
      401,
    );
    const renewed = await f.dispatch("POST", "/v1/session/refresh", {
      body: { refreshToken: session.refreshToken },
    });
    assert.equal(renewed.status, 200);
    assert.notEqual(renewed.json.refreshToken, session.refreshToken);
    assert.equal(
      (
        await f.dispatch("POST", "/v1/session/refresh", {
          body: { refreshToken: session.refreshToken },
        })
      ).status,
      401,
    );
    assert.equal(
      (
        await f.dispatch("GET", "/v1/session", {
          headers: { authorization: "Bearer " + renewed.json.accessToken },
        })
      ).status,
      401,
    );
  } finally {
    f.close();
  }
});
test("sign-out revokes refresh access even when the access token has expired", async () => {
  const f = fixture();
  try {
    const session = await login(f);
    f.advance(901);
    assert.equal(
      (
        await f.dispatch("POST", "/v1/session/logout", {
          body: { refreshToken: session.refreshToken },
        })
      ).status,
      200,
    );
    assert.equal(
      (
        await f.dispatch("POST", "/v1/session/refresh", {
          body: { refreshToken: session.refreshToken },
        })
      ).status,
      401,
    );
  } finally {
    f.close();
  }
});
test("Steam OpenID requires trusted endpoint, signed identity, nonce and positive server verification", async () => {
  const f = fixture();
  try {
    const session = await login(f, "steam");
    assert.equal(session.user.name, "Steam user");
    const ctx = await begin(f, "steam");
    assert.equal(
      (
        await approve(f, ctx, "steam", {
          "openid.op_endpoint": "https://evil.example/",
        })
      ).status,
      401,
    );
    const next = await begin(f, "steam");
    f.invalidSteam();
    assert.equal((await approve(f, next, "steam")).status, 401);
  } finally {
    f.close();
  }
});

test("HTTP transport applies no-store headers and rejects unauthenticated sessions", async () => {
  const f = fixture();
  try {
    await new Promise(resolve => f.server.listen(0, "127.0.0.1", resolve));
    const base = "http://127.0.0.1:" + f.server.address().port;
    const health = await fetch(base + "/health");
    assert.equal(health.status, 200);
    assert.deepEqual(await health.json(), { ok: true });
    assert.equal(health.headers.get("cache-control"), "no-store");
    assert.equal(health.headers.get("access-control-allow-origin"), null);
    const session = await fetch(base + "/v1/session");
    assert.equal(session.status, 401);
    const malformed = await fetch(base + "/v1/login/start", { method: "POST", body: "{" });
    assert.equal(malformed.status, 400);
  } finally {
    f.server.closeAllConnections();
    f.close();
  }
});
