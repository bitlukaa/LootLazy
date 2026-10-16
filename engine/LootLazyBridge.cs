// LootLazy integration adapter. Upstream ASF remains under Apache-2.0.
using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using ArchiSteamFarm.Core;
using ArchiSteamFarm.Steam;
using Net.Codecrete.QrCodeGenerator;

namespace ArchiSteamFarm;

internal static class LootLazyVault {
    private const string Prefix = "LootLazy-DPAPI-v1:";
    internal static bool ProtectedPath(string path) => Path.GetExtension(path) == ".db" && Path.GetFileName(path).StartsWith("ll_", StringComparison.Ordinal);
    internal static string Encode(string path, string value) {
        if (!ProtectedPath(path)) return value;
        byte[] bytes = Encoding.UTF8.GetBytes(value);
        try { return Prefix + Convert.ToBase64String(ProtectedData.Protect(bytes, null, DataProtectionScope.CurrentUser)); }
        finally { CryptographicOperations.ZeroMemory(bytes); }
    }
    internal static string Decode(string path, string value) {
        if (!ProtectedPath(path)) return value;
        if (!value.StartsWith(Prefix, StringComparison.Ordinal)) throw new CryptographicException("Unprotected account database rejected");
        byte[] bytes = ProtectedData.Unprotect(Convert.FromBase64String(value[Prefix.Length..]), null, DataProtectionScope.CurrentUser);
        try { return Encoding.UTF8.GetString(bytes); }
        finally { CryptographicOperations.ZeroMemory(bytes); }
    }
    internal static void CheckPath(string path) {
        string? current = Path.GetFullPath(path);
        while (current != null) {
            if ((File.Exists(current) || Directory.Exists(current)) && (File.GetAttributes(current) & FileAttributes.ReparsePoint) != 0) throw new IOException("Linked state path rejected");
            current = Path.GetDirectoryName(current);
        }
    }
}

internal static class LootLazyBridge {
    internal static bool Enabled => true;
    // Self-test only: inspect anonymous serialized status packets in memory, without connecting or logging in.
    private sealed class PresencePacketFixture : SteamKit2.IDebugNetworkListener {
        internal readonly List<uint> States = [];
        internal bool Invalid;
        public void OnIncomingNetworkMessage(SteamKit2.EMsg type, byte[] data) { }
        public void OnOutgoingNetworkMessage(SteamKit2.EMsg type, byte[] data) {
            try {
                if (type != SteamKit2.EMsg.ClientChangeStatus) { Invalid = true; return; }
                var packet = new SteamKit2.PacketClientMsgProtobuf(type, data);
                var message = new SteamKit2.ClientMsgProtobuf<SteamKit2.Internal.CMsgClientChangeStatus>(packet);
                States.Add(message.Body.persona_state);
            } catch { Invalid = true; }
        }
    }
    private sealed record Account(string Id, string Label, string Login, uint[] Games, string Presence = "online", bool RememberLogin = true, string? AvatarHash = null);
    private static SteamKit2.EPersonaState Persona(string presence) => presence switch {
        "online" => SteamKit2.EPersonaState.Online, "away" => SteamKit2.EPersonaState.Away,
        "invisible" => SteamKit2.EPersonaState.Invisible, "offline" => SteamKit2.EPersonaState.Offline,
        _ => throw new InvalidOperationException("accounts_invalid")
    };
    private sealed record Challenge(string Id, ASF.EUserInputType Type, TaskCompletionSource<string?> Completion);
    private sealed class Session {
        internal uint[] Games = [];
        internal Dictionary<uint, DateTimeOffset> GameStarts = [];
        internal DateTimeOffset Started;
        internal int Minutes;
        internal bool Blocked;
    }
    private static readonly ConcurrentDictionary<string, DateTimeOffset> QrLogins = new(StringComparer.Ordinal);
    internal static bool WantsQrLogin(Bot bot) => QrLogins.TryGetValue(bot.BotName, out var deadline) && deadline > DateTimeOffset.UtcNow;
    private static bool ValidLogin(string login) => login.Length is > 0 and <= 64 && login.All(c => char.IsAsciiLetterOrDigit(c) || c == '_');
    private static string[] EncodeQr(string payload) {
        if (!Uri.TryCreate(payload, UriKind.Absolute, out var uri)) throw new InvalidOperationException("accounts_invalid");
        if (uri.Scheme != "https" || uri.Host != "s.team" || uri.Port != 443 || !string.IsNullOrEmpty(uri.UserInfo) || !uri.AbsolutePath.StartsWith("/q/", StringComparison.Ordinal) || payload.Length > 512) throw new InvalidOperationException("accounts_invalid");
        var code = QrCode.EncodeText(payload, QrCode.Ecc.Medium);
        if (code.Size > 97) throw new InvalidOperationException("accounts_invalid");
        return Enumerable.Range(0, code.Size).Select(y => new string(Enumerable.Range(0, code.Size).Select(x => code.GetModule(x,y) ? '1' : '0').ToArray())).ToArray();
    }
    // Only the protocol thread persists identities; ASF callbacks never mutate account metadata.
    private static async Task BindQrIdentities() {
        foreach (var entry in Accounts.Values.ToArray()) {
            if (Bot.Bots?.TryGetValue(entry.Id, out var bot) != true || !bot.IsConnectedAndLoggedOn || !QrLogins.ContainsKey(entry.Id)) continue;
            string login = bot.BotConfig.SteamLogin ?? "";
            if (!ValidLogin(login) || Accounts.Values.Any(other => other.Id != entry.Id && other.Login.Equals(login, StringComparison.OrdinalIgnoreCase))) {
                AuthErrors[entry.Id] = "accounts_duplicate"; QrLogins.TryRemove(entry.Id, out _); await bot.Stop(true).ConfigureAwait(false); continue;
            }
            if (entry.Login != login) {
                var updated = entry with { Login = login };
                WriteConfig(updated); Accounts[entry.Id] = updated;
                try { Save(); } catch { Accounts[entry.Id] = entry; throw; }
            }
            QrLogins.TryRemove(entry.Id, out _);
        }
        foreach (string id in Restoring.Keys) {
            if (!Accounts.TryGetValue(id, out var entry) || Bot.Bots?.TryGetValue(id, out var bot) != true || !bot.IsConnectedAndLoggedOn) continue;
            await bot.FlushLootLazyLogin().ConfigureAwait(false);
            Accounts[id] = entry with { RememberLogin = true };
            try { Save(); } catch { Accounts[id] = entry; throw; }
            Restoring.TryRemove(id, out _);
        }
    }
    private sealed record QrChallenge(string Attempt, string Payload, string Revision);
    private static readonly ConcurrentDictionary<string, QrChallenge> QrCodes = new(StringComparer.Ordinal);
    private static readonly ConcurrentDictionary<string, string> AuthErrors = new(StringComparer.Ordinal);
    private static readonly ConcurrentDictionary<string, ConcurrentDictionary<string, ConnectionFlow>> ConnectionFlows = new(StringComparer.Ordinal);
    internal sealed class ConnectionFlow : IDisposable {
        private readonly ConcurrentDictionary<string, ConnectionFlow> Owner;
        private readonly string Id = Guid.NewGuid().ToString("N");
        private readonly CancellationTokenSource Cancellation = new();
        internal readonly TaskCompletionSource Completion = new(TaskCreationOptions.RunContinuationsAsynchronously);
        internal CancellationToken Token => Cancellation.Token;
        internal string? QrAttempt { get; }
        internal ConnectionFlow(string id, string? attempt) { Owner = ConnectionFlows.GetOrAdd(id, _ => new(StringComparer.Ordinal)); QrAttempt = attempt; Owner[Id] = this; }
        internal void Cancel() { try { Cancellation.Cancel(); } catch (ObjectDisposedException) { } }
        public void Dispose() { Owner.TryRemove(Id, out _); Cancellation.Dispose(); Completion.TrySetResult(); }
    }
    internal static ConnectionFlow BeginConnection(Bot bot) => new(bot.BotName, QrAttemptId(bot));
    private static void CancelConnections(string id) { if (ConnectionFlows.TryGetValue(id, out var flows)) foreach (var flow in flows.Values) flow.Cancel(); }
    private static async Task WaitConnections(string id) {
        if (!ConnectionFlows.TryGetValue(id, out var flows)) return;
        try { await Task.WhenAll(flows.Values.Select(flow => flow.Completion.Task)).WaitAsync(TimeSpan.FromSeconds(12)).ConfigureAwait(false); }
        catch (TimeoutException) { Fail("accounts_auth_busy"); }
    }
    private static readonly ConcurrentDictionary<string, byte> Restoring = new(StringComparer.Ordinal);
    private static bool CanResume(Account account, Bot? bot) => !Drafts.Contains(account.Id) && bot?.HasLootLazyLogin == true && !AuthErrors.ContainsKey(account.Id);
    private static readonly HashSet<string> Drafts = new(StringComparer.Ordinal);
    internal static string? QrAttemptId(Bot bot) => QrLogins.TryGetValue(bot.BotName, out var deadline) ? deadline.Ticks.ToString(System.Globalization.CultureInfo.InvariantCulture) : null;
    internal static void PublishQr(Bot bot, string? attempt, string payload) {
        if (attempt == null || attempt != QrAttemptId(bot) || !WantsQrLogin(bot)) return;
        QrCodes[bot.BotName] = new QrChallenge(attempt, payload, Guid.NewGuid().ToString("N"));
    }
    internal static void ClearQr(Bot bot, string? attempt) {
        if (QrCodes.TryGetValue(bot.BotName, out var qr) && qr.Attempt == attempt) ((ICollection<KeyValuePair<string, QrChallenge>>)QrCodes).Remove(new(bot.BotName, qr));
    }
    internal static void QrMismatch(Bot bot) => AuthErrors[bot.BotName] = "accounts_qr_mismatch";
    internal static bool QrFailed(Bot bot, string? attempt) {
        if (attempt == null || QrAttemptId(bot) != attempt) return false;
        AuthErrors.TryAdd(bot.BotName, "accounts_qr_failed");
        QrLogins.TryRemove(bot.BotName, out _); QrCodes.TryRemove(bot.BotName, out _); return true;
    }
    private static bool IsBusy() => Drafts.Count > 0 || Challenges.Count > 0 || Sessions.Values.Any(session => session.Games.Length > 0) || (Bot.Bots?.Values.Any(bot => bot.KeepRunning && !bot.IsConnectedAndLoggedOn) ?? false);
    private static async Task EndSession(string id, Func<Task> send, Session? expected = null) {
        if (expected != null) {
            if (!((ICollection<KeyValuePair<string, Session>>)Sessions).Remove(new(id, expected))) return;
            expected.Games = [];
        } else if (Sessions.TryRemove(id, out var session)) session.Games = [];
        await send().ConfigureAwait(false);
    }
    private static async Task UpdateSessionGames(string id, uint[] games, Func<Task> send) {
        if (!Sessions.TryGetValue(id, out var previous) || previous.Games.Length == 0) return;
        var now = DateTimeOffset.UtcNow;
        var updated = new Session {
            Games = games.ToArray(), Started = previous.Started, Minutes = previous.Minutes, Blocked = previous.Blocked,
            GameStarts = games.ToDictionary(game => game, game => previous.GameStarts.TryGetValue(game, out var started) ? started : previous.Games.Contains(game) ? previous.Started : now)
        };
        // An expiry or Stop that already ended this session must not be undone by an edit.
        if (!Sessions.TryUpdate(id, updated, previous)) return;
        try { await send().ConfigureAwait(false); }
        catch { Sessions.TryUpdate(id, previous, updated); throw; }
        if (games.Length == 0) ((ICollection<KeyValuePair<string, Session>>)Sessions).Remove(new(id, updated));
    }
    private static readonly Dictionary<string, Account> Accounts = new(StringComparer.Ordinal);
    private static readonly ConcurrentDictionary<string, Challenge> Challenges = new(StringComparer.Ordinal);
    private sealed record Library(ulong SteamId, Dictionary<uint, string> Games);
    private static readonly ConcurrentDictionary<string, Library> Libraries = new(StringComparer.Ordinal);
    private static readonly ConcurrentDictionary<string, string> LibraryRequests = new(StringComparer.Ordinal);
    private static readonly ConcurrentDictionary<string, byte> LibraryErrors = new(StringComparer.Ordinal);
    private static readonly ConcurrentDictionary<string, Session> Sessions = new(StringComparer.Ordinal);
    private static readonly JsonSerializerOptions JsonOptions = new() { PropertyNamingPolicy = JsonNamingPolicy.CamelCase };
    private static readonly SemaphoreSlim PlayLock = new(1, 1);
    private static bool OfflineTest;
    private static string Metadata => Path.Combine(Directory.GetCurrentDirectory(), "accounts.json");
    private static void Fail(string code) => throw new InvalidOperationException(code);
    private static string Text(JsonElement body, string key) => body.ValueKind == JsonValueKind.Object && body.TryGetProperty(key, out JsonElement value) && value.ValueKind == JsonValueKind.String ? value.GetString()! : "";
    private static bool ValidId(string id) => id.StartsWith("ll_", StringComparison.Ordinal) && id.Length == 35 && id[3..].All(Uri.IsHexDigit);
    private static string ConfigPath(string id) => Path.Combine("config", id + ".json");
    private static void Save() {
        LootLazyVault.CheckPath(Metadata);
        LootLazyVault.CheckPath(Metadata + ".new");
        File.WriteAllText(Metadata + ".new", JsonSerializer.Serialize(Accounts.Values.Where(account => !Drafts.Contains(account.Id)), JsonOptions));
        File.Move(Metadata + ".new", Metadata, true);
    }
    private static void WriteConfig(Account account) {
        string path = ConfigPath(account.Id);
        LootLazyVault.CheckPath(path);
        File.WriteAllText(path, JsonSerializer.Serialize(new { Enabled = false, MachineName = "LootLazy", SteamLogin = account.Login, OnlineStatus = (int)Persona(account.Presence), UseLoginKeys = true, FarmingPreferences = 9, RemoteCommunication = 0, BotBehaviour = 64, TradeCheckPeriod = 0, AcceptGifts = false, GamesPlayedWhileIdle = Array.Empty<uint>() }));
    }
    internal static async Task<string?> InputAsync(ASF.EUserInputType type, string id) {
        if (!ValidId(id) || Bot.Bots?.TryGetValue(id, out var inputBot) != true || !inputBot.KeepRunning) return null;
        if (Restoring.ContainsKey(id)) { AuthErrors[id] = "accounts_signin_required"; return null; }
        if (type == ASF.EUserInputType.QrCodeLogin) return WantsQrLogin(inputBot) ? "Y" : "N";
        var challenge = new Challenge(Guid.NewGuid().ToString("N"), type, new(TaskCreationOptions.RunContinuationsAsynchronously));
        if (Challenges.TryRemove(id, out var old)) old.Completion.TrySetResult(null);
        Challenges[id] = challenge;
        try { return await challenge.Completion.Task.WaitAsync(TimeSpan.FromMinutes(3)).ConfigureAwait(false); }
        catch (TimeoutException) { return null; }
        finally { if (Challenges.TryGetValue(id, out var current) && current.Id == challenge.Id) Challenges.TryRemove(id, out _); }
    }
    internal static void StopRequested(Bot bot) {
        if (Sessions.TryGetValue(bot.BotName, out var session)) session.Games = [];
    }
    internal static void Occupied(Bot bot) {
        if (Sessions.TryGetValue(bot.BotName, out var session)) { session.Games = []; session.Blocked = true; }
    }
    internal static async Task ResetGames(Bot bot) {
        if (!bot.IsConnectedAndLoggedOn) return;
        await PlayLock.WaitAsync().ConfigureAwait(false);
        try {
            uint[] games = Sessions.TryGetValue(bot.BotName, out var session) ? session.Games : [];
            if (!bot.IsPlayingPossible) { Occupied(bot); return; }
            await bot.ArchiHandler.PlayGames(games).ConfigureAwait(false);
        } finally { PlayLock.Release(); }
    }
    private static bool ValidAvatar(string? hash) => hash?.Length == 40 && hash.All(Uri.IsHexDigit);
    private static void RefreshAvatars() {
        var changed = new Dictionary<string, Account>();
        foreach (var account in Accounts.Values.ToArray()) {
            if (Bot.Bots?.TryGetValue(account.Id, out var bot) != true || !bot.IsConnectedAndLoggedOn || !ValidAvatar(bot.AvatarHash) || account.AvatarHash == bot.AvatarHash) continue;
            changed[account.Id] = account;
            Accounts[account.Id] = account with { AvatarHash = bot.AvatarHash };
        }
        if (changed.Count == 0) return;
        try { Save(); } catch { foreach (var entry in changed) Accounts[entry.Key] = entry.Value; }
    }
    private static async Task WaitForLibrary(Func<bool> ready, Action begin, Func<bool> failed, Func<bool> connected, TimeSpan timeout) {
        if (ready()) return;
        begin();
        var watch = System.Diagnostics.Stopwatch.StartNew();
        while (watch.Elapsed < timeout) {
            if (!connected()) Fail("accounts_not_ready");
            if (ready()) return;
            if (failed()) Fail("accounts_library_failed");
            await Task.Delay(50).ConfigureAwait(false);
        }
        Fail("accounts_library_failed");
    }
    private static Task EnsureLibrary(Bot bot) {
        string id = bot.BotName;
        return WaitForLibrary(
            () => Libraries.TryGetValue(id, out var library) && library.SteamId == bot.SteamID,
            () => { LibraryErrors.TryRemove(id, out _); LibraryResult(bot, JsonSerializer.SerializeToElement(new { })); },
            () => LibraryErrors.ContainsKey(id) || !LibraryRequests.ContainsKey(id),
            () => bot.IsConnectedAndLoggedOn, TimeSpan.FromSeconds(17));
    }
    private static object Snapshot() {
        var result = Accounts.Values.Select(account => {
            Bot? bot = null;
            Bot.Bots?.TryGetValue(account.Id, out bot);
            Challenges.TryGetValue(account.Id, out var input);
            Sessions.TryGetValue(account.Id, out var session);
            bool connected = bot?.IsConnectedAndLoggedOn == true;
            bool running = connected && bot!.IsPlayingPossible && session?.Games.Length > 0;
            string state = input != null ? "input" : running ? "running" : connected && !bot!.IsPlayingPossible ? "blocked" : connected ? "ready" : bot?.KeepRunning == true ? "connecting" : "offline";
            string? avatarHash = ValidAvatar(bot?.AvatarHash) ? bot!.AvatarHash : ValidAvatar(account.AvatarHash) ? account.AvatarHash : null;
            uint playingAppId = connected && !bot!.IsPlayingPossible ? bot.SteamFriends.GetFriendGamePlayed(bot.SteamID).AppID : 0;
            string? playingGameName = playingAppId > 0 ? bot!.SteamFriends.GetFriendGamePlayedName(bot.SteamID) : null;
            return new { avatarUrl = avatarHash == null ? null : "https://avatars.steamstatic.com/" + avatarHash.ToLowerInvariant() + "_medium.jpg", playingAppId, playingGameName, runningGameCount = running ? session!.Games.Length : 0, id = account.Id, label = account.Label, login = account.Login, draft = Drafts.Contains(account.Id), games = account.Games, presence = account.Presence, rememberLogin = account.RememberLogin, canResume = CanResume(account, bot), state, connected, active = bot?.KeepRunning == true, steamId = bot?.SSteamID ?? "0", input = input == null ? null : new { id = input.Id, type = input.Type.ToString() }, elapsedSeconds = running ? Math.Max(0, (long)(DateTimeOffset.UtcNow - session!.Started).TotalSeconds) : 0, minutes = session?.Minutes ?? 0, gameElapsedSeconds = running ? session!.Games.ToDictionary(game => game, game => Math.Max(0, (long)(DateTimeOffset.UtcNow - (session.GameStarts.TryGetValue(game, out var started) ? started : session.Started)).TotalSeconds)) : new Dictionary<uint, long>(), error = AuthErrors.TryGetValue(account.Id, out var authError) ? authError : bot == null && !OfflineTest ? "accounts_restore_failed" : bot != null && !bot.KeepRunning && bot.LastLogOnResult is not (SteamKit2.EResult.Invalid or SteamKit2.EResult.OK) ? "accounts_login_failed" : null };
        }).ToArray();
        return new { protocol = 1, accounts = result, active = IsBusy() };
    }
    private static Account Find(JsonElement body) {
        string id = Text(body, "id");
        if (!ValidId(id) || !Accounts.TryGetValue(id, out var account)) { Fail("accounts_missing"); return null!; }
        return account;
    }
    private static object LibraryResult(Bot bot, JsonElement body) {
        string id = bot.BotName, query = Text(body, "query").Trim();
        if (query.Length > 160) { Fail("accounts_invalid"); return null!; }
        if (body.TryGetProperty("refresh", out var refresh) && refresh.ValueKind == JsonValueKind.True && !LibraryRequests.ContainsKey(id)) { Libraries.TryRemove(id, out _); LibraryErrors.TryRemove(id, out _); }
        HashSet<uint>? requested = null;
        if (body.TryGetProperty("appIds", out var ids)) {
            if (ids.ValueKind != JsonValueKind.Array || ids.GetArrayLength() > 30) Fail("accounts_invalid");
            requested = new HashSet<uint>();
            foreach (var value in ids.EnumerateArray()) {
                if (value.ValueKind != JsonValueKind.Number || !value.TryGetUInt32(out uint game) || game == 0) Fail("accounts_invalid");
                requested.Add(value.GetUInt32());
            }
        }
        if (Libraries.TryGetValue(id, out var library) && library.SteamId == bot.SteamID) {
            var games = library.Games.Where(game => requested != null ? requested.Contains(game.Key) : query.Length == 0 || game.Value.Contains(query, StringComparison.OrdinalIgnoreCase) || game.Key.ToString(System.Globalization.CultureInfo.InvariantCulture).Contains(query, StringComparison.Ordinal)).OrderBy(game => game.Value, StringComparer.OrdinalIgnoreCase).Take(50).Select(game => new { id = game.Key, name = game.Value[..Math.Min(game.Value.Length, 160)] }).ToArray();
            return new { games, total = library.Games.Count, loading = false, active = IsBusy(), error = (string?)null };
        }
        if (!LibraryErrors.ContainsKey(id)) {
            string nonce = Guid.NewGuid().ToString("N");
            if (LibraryRequests.TryAdd(id, nonce)) {
                ulong steamId = bot.SteamID;
                _ = Task.Run(async () => {
                    try {
                        var games = await bot.ArchiHandler.GetOwnedGames(steamId).WaitAsync(TimeSpan.FromSeconds(15)).ConfigureAwait(false);
                        if (!LibraryRequests.TryGetValue(id, out var current) || current != nonce) return;
                        if (games == null || !bot.IsConnectedAndLoggedOn || bot.SteamID != steamId) { LibraryErrors[id] = 1; return; }
                        Libraries[id] = new Library(steamId, games.Where(game => game.Key > 0 && !string.IsNullOrWhiteSpace(game.Value)).Take(100000).ToDictionary(game => game.Key, game => game.Value));
                    } catch { if (LibraryRequests.TryGetValue(id, out var current) && current == nonce) LibraryErrors[id] = 1; }
                    finally { if (LibraryRequests.TryGetValue(id, out var current) && current == nonce) LibraryRequests.TryRemove(id, out _); }
                });
            }
        }
        return new { games = Array.Empty<object>(), total = 0, loading = LibraryRequests.ContainsKey(id), active = IsBusy(), error = LibraryErrors.ContainsKey(id) ? "accounts_library_failed" : null };
    }
    private static async Task<object> Handle(string command, JsonElement body) {
        if (!new[] { "status", "add", "remove", "games", "signin", "input", "start", "stop", "library", "qr", "commit", "disconnect", "presence", "resume", "rename" }.Contains(command, StringComparer.Ordinal)) Fail("accounts_invalid");
        await BindQrIdentities().ConfigureAwait(false);
        RefreshAvatars();
        if (command == "status") return Snapshot();
        if (command == "add") {
            string label = Text(body, "label").Trim(), login = Text(body, "login").Trim();
            if (Accounts.Count >= 20 || label.Length is < 1 or > 48 || label.Any(char.IsControl) || (!ValidLogin(login) && !(login.Length == 0 && Text(body, "method") == "qr"))) Fail("accounts_invalid");
            if (login.Length > 0 && Accounts.Values.Any(account => account.Login.Equals(login, StringComparison.OrdinalIgnoreCase))) Fail("accounts_duplicate");
            var account = new Account("ll_" + Guid.NewGuid().ToString("N"), label, login, [], RememberLogin: false);
            WriteConfig(account);
            if (!OfflineTest) await Bot.RegisterBot(account.Id).ConfigureAwait(false);
            Accounts.Add(account.Id, account);
            if (body.TryGetProperty("draft", out var draft) && draft.ValueKind == JsonValueKind.True) Drafts.Add(account.Id);
            Save();
            return Snapshot();
        }
        var entry = Find(body);
        Bot? selected = null;
        Bot.Bots?.TryGetValue(entry.Id, out selected);
        if (command == "remove") {
            QrLogins.TryRemove(entry.Id, out _); QrCodes.TryRemove(entry.Id, out _); AuthErrors.TryRemove(entry.Id, out _);
            if (Challenges.TryRemove(entry.Id, out var pending)) pending.Completion.TrySetResult(null);
            Restoring.TryRemove(entry.Id, out _);
            Sessions.TryRemove(entry.Id, out _);
            Libraries.TryRemove(entry.Id, out _); LibraryRequests.TryRemove(entry.Id, out _); LibraryErrors.TryRemove(entry.Id, out _);
            if (selected != null) {
                CancelConnections(entry.Id);
                await selected.Stop(true).ConfigureAwait(false);
                await WaitConnections(entry.Id).ConfigureAwait(false);
                if (!await selected.DeleteAllRelatedFiles().ConfigureAwait(false)) Fail("accounts_remove_failed");
            }
            foreach (string suffix in new[] { ".json", ".db", ".db.new", ".json.new" }) {
                string ownedFile = Path.Combine("config", entry.Id + suffix);
                LootLazyVault.CheckPath(ownedFile);
                File.Delete(ownedFile);
            }
            if (selected != null) await selected.Destroy(true).ConfigureAwait(false);
            Accounts.Remove(entry.Id); Drafts.Remove(entry.Id); ConnectionFlows.TryRemove(entry.Id, out _);
            Save();
            return Snapshot();
        }
        if (command == "rename") {
            string label = Text(body, "label").Trim();
            if (Drafts.Contains(entry.Id) || label.Length is < 1 or > 48 || label.Any(char.IsControl)) Fail("accounts_invalid");
            Accounts[entry.Id] = entry with { Label = label };
            try { Save(); } catch { Accounts[entry.Id] = entry; throw; }
            return Snapshot();
        }
        if (command == "games") {
            if ((selected?.KeepRunning == true && !selected.IsConnectedAndLoggedOn) || Challenges.ContainsKey(entry.Id)) Fail("accounts_not_ready");
            if (!body.TryGetProperty("games", out var values) || values.ValueKind != JsonValueKind.Array || values.GetArrayLength() > 32) Fail("accounts_invalid");
            var parsed = new List<uint>();
            foreach (var value in values.EnumerateArray()) {
                if (value.ValueKind != JsonValueKind.Number || !value.TryGetUInt32(out uint game) || game == 0) Fail("accounts_invalid");
                parsed.Add(value.GetUInt32());
            }
            uint[] games = parsed.Distinct().ToArray();
            if (!OfflineTest && (selected?.IsConnectedAndLoggedOn != true || !Libraries.TryGetValue(entry.Id, out var owned) || owned.SteamId != selected.SteamID || games.Any(id => !owned.Games.ContainsKey(id)))) Fail("accounts_not_owned");
            if (Sessions.TryGetValue(entry.Id, out var playing) && playing.Games.Length > 0 && selected?.IsPlayingPossible != true) Fail("accounts_playing_elsewhere");
            Accounts[entry.Id] = entry with { Games = games };
            try { Save(); } catch { Accounts[entry.Id] = entry; throw; }
            try { await UpdateSessionGames(entry.Id, games, () => selected == null ? Task.CompletedTask : ResetGames(selected)).ConfigureAwait(false); }
            catch { Accounts[entry.Id] = entry; Save(); throw; }
            return Snapshot();
        }
        if (command == "presence") {
            string presence = Text(body, "presence");
            var persona = Persona(presence);
            Accounts[entry.Id] = entry with { Presence = presence };
            try { Save(); } catch { Accounts[entry.Id] = entry; throw; }
            if (selected != null) {
                selected.BotConfig.OnlineStatus = persona;
                if (selected.IsConnectedAndLoggedOn) selected.ResetPersonaState();
            }
            return Snapshot();
        }
        if (OfflineTest || selected == null) { Fail(OfflineTest ? "accounts_not_ready" : "accounts_restore_failed"); return null!; }
        if (command == "qr") {
            QrCodes.TryGetValue(entry.Id, out var qr);
            bool pending = WantsQrLogin(selected) && selected.KeepRunning && !selected.IsConnectedAndLoggedOn;
            string[]? rows = pending && qr != null && qr.Attempt == QrAttemptId(selected) ? EncodeQr(qr.Payload) : null;
            return new { rows, revision = rows == null ? null : qr!.Revision, pending, active = IsBusy() };
        }
        if (command == "library") { if (!selected.IsConnectedAndLoggedOn) { Fail("accounts_not_ready"); return null!; } return LibraryResult(selected, body); }
        if (command == "commit") {
            if (!selected.IsConnectedAndLoggedOn || !ValidLogin(entry.Login)) Fail("accounts_not_ready");
            string label = Drafts.Contains(entry.Id) ? Text(body, "label").Trim() : entry.Label;
            if (label.Length is < 1 or > 48 || label.Any(char.IsControl)) Fail("accounts_invalid");
            // Persist the protected login database before reporting that sign-in has been saved.
            await selected.FlushLootLazyLogin().ConfigureAwait(false);
            bool draft = Drafts.Remove(entry.Id);
            Accounts[entry.Id] = entry with { Label = label, RememberLogin = true };
            try { Save(); } catch { if (draft) Drafts.Add(entry.Id); Accounts[entry.Id] = entry; throw; }
            Restoring.TryRemove(entry.Id, out _);
            return Snapshot();
        }
        if (command == "resume") {
            if (!CanResume(entry, selected) || selected.KeepRunning) Fail("accounts_signin_required");
            await WaitConnections(entry.Id).ConfigureAwait(false);
            QrLogins.TryRemove(entry.Id, out _); QrCodes.TryRemove(entry.Id, out _);
            Restoring[entry.Id] = 1;
            selected.BotConfig.OnlineStatus = Persona(entry.Presence);
            await selected.Start().ConfigureAwait(false);
            return Snapshot();
        }
        if (command == "signin") {
            if (selected.KeepRunning) Fail("accounts_stop_first");
            await WaitConnections(entry.Id).ConfigureAwait(false);
            Restoring.TryRemove(entry.Id, out _);
            AuthErrors.TryRemove(entry.Id, out _); QrCodes.TryRemove(entry.Id, out _);
            Sessions.TryRemove(entry.Id, out _);
            Libraries.TryRemove(entry.Id, out _); LibraryRequests.TryRemove(entry.Id, out _); LibraryErrors.TryRemove(entry.Id, out _);
            string password = Text(body, "password");
            if (password.Length > 256) Fail("accounts_invalid");
            bool qr = Text(body, "method") == "qr";
            if (!qr && entry.Login.Length == 0) {
                string login = Text(body, "login").Trim();
                if (!ValidLogin(login)) Fail("accounts_invalid");
                if (Accounts.Values.Any(other => other.Id != entry.Id && other.Login.Equals(login, StringComparison.OrdinalIgnoreCase))) Fail("accounts_duplicate");
                entry = entry with { Login = login }; WriteConfig(entry); Accounts[entry.Id] = entry; Save();
            }
            if (!qr && !ValidLogin(entry.Login)) Fail("accounts_invalid");
            if (qr) QrLogins[entry.Id] = DateTimeOffset.UtcNow.AddMinutes(3); else QrLogins.TryRemove(entry.Id, out _);
            selected.BotConfig.SteamLogin = qr && Drafts.Contains(entry.Id) ? "" : entry.Login;
            selected.BotConfig.IsSteamLoginSet = false;
            selected.BotConfig.OnlineStatus = Persona(entry.Presence);
            if (password.Length > 0) selected.SetUserInput(ASF.EUserInputType.Password, password);
            await selected.Start().ConfigureAwait(false);
        } else if (command == "input") {
            if (!Challenges.TryGetValue(entry.Id, out var challenge) || challenge.Id != Text(body, "challengeId") || challenge.Completion.Task.IsCompleted) Fail("accounts_input_expired");
            string value = Text(body, "value");
            if (value.Length is < 1 or > 256) Fail("accounts_invalid");
            challenge!.Completion.TrySetResult(value);
            // Status clears after ASF consumes and validates the input; never echo it.
        } else if (command == "start") {
            if (!selected.IsConnectedAndLoggedOn) Fail("accounts_not_ready");
            if (!selected.IsPlayingPossible) Fail("accounts_playing_elsewhere");
            if (entry.Games.Length == 0) Fail("accounts_choose_games");
            await EnsureLibrary(selected).ConfigureAwait(false);
            if (!selected.IsPlayingPossible) Fail("accounts_playing_elsewhere");
            foreach (uint id in entry.Games) {
                if (!Libraries.TryGetValue(entry.Id, out var owned) || owned.SteamId != selected.SteamID || !owned.Games.ContainsKey(id)) Fail("accounts_not_owned");
            }
            int minutes = body.TryGetProperty("minutes", out var duration) ? duration.GetInt32() : 0;
            if (minutes is < 0 or > 1440) Fail("accounts_invalid");
            if (Sessions.TryGetValue(entry.Id, out var activeSession) && activeSession.Games.Length > 0) return Snapshot();
            var started = DateTimeOffset.UtcNow;
            Sessions[entry.Id] = new Session { Games = entry.Games.ToArray(), Started = started, Minutes = minutes, GameStarts = entry.Games.ToDictionary(game => game, _ => started) };
            try { await ResetGames(selected).ConfigureAwait(false); }
            catch { Sessions.TryRemove(entry.Id, out _); throw; }
        } else if (command == "stop") {
            // Stop is a game-session action. The authenticated connection and library stay available.
            try { await EndSession(entry.Id, () => ResetGames(selected)).ConfigureAwait(false); }
            catch { await selected.Stop(true).ConfigureAwait(false); throw; }
        } else if (command == "disconnect") {
            if (body.TryGetProperty("signOut", out var signOut) && signOut.ValueKind == JsonValueKind.True) {
                Accounts[entry.Id] = entry with { RememberLogin = false };
                try { Save(); } catch { Accounts[entry.Id] = entry; throw; }
            }
            Restoring.TryRemove(entry.Id, out _);
            CancelConnections(entry.Id);
            QrLogins.TryRemove(entry.Id, out _); QrCodes.TryRemove(entry.Id, out _);
            if (Challenges.TryRemove(entry.Id, out var pending)) pending.Completion.TrySetResult(null);
            selected.SetUserInput(ASF.EUserInputType.QrCodeLogin, "N");
            try { await EndSession(entry.Id, () => ResetGames(selected)).ConfigureAwait(false); }
            finally { await selected.Stop(true).ConfigureAwait(false); await WaitConnections(entry.Id).ConfigureAwait(false); }
        } else { Fail("accounts_invalid"); }
        return Snapshot();
    }
    private static async Task ExpireSessions(CancellationToken token) {
        while (!token.IsCancellationRequested) {
            await Task.Delay(1000, token).ConfigureAwait(false);
            foreach (var pair in QrLogins) {
                if (pair.Value > DateTimeOffset.UtcNow || (Bot.Bots?.TryGetValue(pair.Key, out var connectedBot) == true && connectedBot.IsConnectedAndLoggedOn)) continue;
                if (((ICollection<KeyValuePair<string, DateTimeOffset>>)QrLogins).Remove(pair) && Bot.Bots?.TryGetValue(pair.Key, out var qrBot) == true && !qrBot.IsConnectedAndLoggedOn) {
                    QrCodes.TryRemove(pair.Key, out _); AuthErrors[pair.Key] = "accounts_qr_expired"; CancelConnections(pair.Key);
                    qrBot.SetUserInput(ASF.EUserInputType.QrCodeLogin, "N");
                    try { await qrBot.Stop(true).ConfigureAwait(false); } catch { Environment.Exit(2); }
                }
            }
            foreach (var pair in Sessions) {
                var session = pair.Value;
                if (session.Minutes == 0 || session.Games.Length == 0 || DateTimeOffset.UtcNow - session.Started < TimeSpan.FromMinutes(session.Minutes)) continue;
                session.Games = [];
                if (Bot.Bots?.TryGetValue(pair.Key, out var bot) == true) {
                    try { await EndSession(pair.Key, () => ResetGames(bot), session).ConfigureAwait(false); }
                    catch { try { await bot.Stop(true).ConfigureAwait(false); } catch { Environment.Exit(2); } }
                }
            }
        }
    }
    private static async Task<int> Main(string[] args) {
        using var output = new StreamWriter(Console.OpenStandardOutput(), new UTF8Encoding(false)) { AutoFlush = true };
        Console.SetOut(TextWriter.Null);
        Console.SetError(TextWriter.Null);
        if (args.Length == 1 && args[0] == "--qr-fixture") {
            const string payload = "https://s.team/q/1/1234567890123456789";
            await output.WriteLineAsync(JsonSerializer.Serialize(new { payload, rows = EncodeQr(payload) })).ConfigureAwait(false); return 0;
        }
        if (args.Length == 1 && args[0] == "--self-test") {
            int libraryStarts = 0; bool libraryReady = false;
            Task? loadingFixture = null;
            await WaitForLibrary(() => libraryReady, () => { libraryStarts++; loadingFixture = Task.Run(async () => { await Task.Delay(80).ConfigureAwait(false); libraryReady = true; }); }, () => false, () => true, TimeSpan.FromSeconds(2)).ConfigureAwait(false);
            if (!libraryReady || libraryStarts != 1) return 17;
            await loadingFixture!.ConfigureAwait(false);
            await WaitForLibrary(() => true, () => { libraryStarts++; }, () => false, () => true, TimeSpan.FromSeconds(1)).ConfigureAwait(false);
            if (libraryStarts != 1) return 18;
            foreach (bool disconnected in new[] { false, true }) {
                try { await WaitForLibrary(() => false, () => {}, () => true, () => !disconnected, TimeSpan.FromSeconds(1)).ConfigureAwait(false); return 19; }
                catch (InvalidOperationException error) { if (error.Message != (disconnected ? "accounts_not_ready" : "accounts_library_failed")) return 20; }
            }
            var statusPackets = new PresencePacketFixture();
            var isolatedClient = new SteamKit2.SteamClient { DebugNetworkListener = statusPackets };
            var friends = isolatedClient.GetHandler<SteamKit2.SteamFriends>();
            if (friends == null) return 14;
            foreach (string presence in new[] { "online", "away", "invisible", "offline" }) friends.SetPersonaState(Persona(presence));
            if (statusPackets.Invalid || !statusPackets.States.SequenceEqual(new uint[] { 1, 3, 7, 0 })) return 15;
            if (isolatedClient.IsConnected || friends.GetPersonaState() != SteamKit2.EPersonaState.Offline) return 16;
            var rows = EncodeQr("https://s.team/q/1/fixture");
            if (rows.Length < 21 || rows.Length > 97 || rows.Any(row => row.Length != rows.Length || row.Any(bit => bit is not ('0' or '1')))) return 7;
            foreach (var invalid in new[] { "http://s.team/q/fixture", "https://example.com/q/fixture", "https://s.team/not-a-challenge", "https://x@s.team/q/fixture" }) {
                try { EncodeQr(invalid); return 8; } catch (InvalidOperationException) { }
            }
            using (var flow = new ConnectionFlow("flow-fixture", null)) {
                CancelConnections("flow-fixture"); if (!flow.Token.IsCancellationRequested) return 10;
            }
            await WaitConnections("flow-fixture").ConfigureAwait(false); ConnectionFlows.TryRemove("flow-fixture", out _);
            int resetCount = 0;
            var superseded = new Session { Games = [730] };
            var currentSession = new Session { Games = [570] };
            Sessions["fixture"] = currentSession;
            await EndSession("fixture", () => { resetCount++; return Task.CompletedTask; }, superseded).ConfigureAwait(false);
            if (!Sessions.TryGetValue("fixture", out var remaining) || remaining != currentSession || resetCount != 0) return 11;
            await EndSession("fixture", () => { resetCount++; return Task.CompletedTask; }).ConfigureAwait(false);
            if (Sessions.ContainsKey("fixture") || resetCount != 1) return 9;
            var sessionStarted = DateTimeOffset.UtcNow.AddMinutes(-5);
            var liveSession = new Session { Games = [730], Started = sessionStarted, Minutes = 15, GameStarts = new() { [730] = sessionStarted } };
            Sessions["live-fixture"] = liveSession;
            uint[] sentGames = [];
            Task CaptureGames() { sentGames = Sessions.TryGetValue("live-fixture", out var active) ? active.Games.ToArray() : []; return Task.CompletedTask; }
            await UpdateSessionGames("live-fixture", [730, 570], CaptureGames).ConfigureAwait(false);
            var expanded = Sessions["live-fixture"];
            if (!sentGames.SequenceEqual(new uint[] { 730, 570 }) || expanded.Started != sessionStarted || expanded.Minutes != 15 || expanded.GameStarts[730] != sessionStarted || expanded.GameStarts[570] < DateTimeOffset.UtcNow.AddSeconds(-5)) return 17;
            await EndSession("live-fixture", () => throw new InvalidOperationException("Stale expiry sent a packet"), liveSession).ConfigureAwait(false);
            if (Sessions["live-fixture"] != expanded) return 18;
            try { await UpdateSessionGames("live-fixture", [570], () => throw new IOException("Fixture send failed")).ConfigureAwait(false); return 19; } catch (IOException) { }
            if (Sessions["live-fixture"] != expanded) return 20;
            await UpdateSessionGames("live-fixture", [570], CaptureGames).ConfigureAwait(false);
            if (!sentGames.SequenceEqual(new uint[] { 570 }) || Sessions["live-fixture"].GameStarts[570] != expanded.GameStarts[570]) return 21;
            await UpdateSessionGames("live-fixture", [], CaptureGames).ConfigureAwait(false);
            if (sentGames.Length != 0 || Sessions.ContainsKey("live-fixture")) return 22;
            await UpdateSessionGames("live-fixture", [730], () => throw new InvalidOperationException("Stopped session restarted")).ConfigureAwait(false);
            const string secret = "{\"BackingRefreshToken\":\"fixture-refresh-token\",\"BackingSteamGuardData\":\"fixture-guard-secret\"}";
            string encrypted = LootLazyVault.Encode("ll_test.db", secret);
            if (encrypted.Contains("fixture", StringComparison.Ordinal) || LootLazyVault.Decode("ll_test.db", encrypted) != secret) return 2;
            try { LootLazyVault.Decode("ll_test.db", secret); return 3; } catch (CryptographicException) { }
            string fixture = Path.Combine(Path.GetTempPath(), "lootlazy-vault-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(fixture);
            try {
                string database = Path.Combine(fixture, "ll_fixture.db");
                if (!await Helpers.SerializableFile.Write(database, secret).ConfigureAwait(false)) return 4;
                string persisted = await File.ReadAllTextAsync(database).ConfigureAwait(false);
                if (persisted.Contains("fixture-refresh", StringComparison.Ordinal) || LootLazyVault.Decode(database, persisted) != secret || File.Exists(database + ".new")) return 5;
                var loaded = await Steam.Storage.BotDatabase.CreateOrLoad(database).ConfigureAwait(false);
                if (loaded == null) return 6;
                loaded.RefreshToken = "rotated-fixture-token";
                await loaded.FlushLootLazyState().ConfigureAwait(false);
                persisted = await File.ReadAllTextAsync(database).ConfigureAwait(false);
                if (persisted.Contains("rotated-fixture-token", StringComparison.Ordinal) || !LootLazyVault.Decode(database, persisted).Contains("rotated-fixture-token", StringComparison.Ordinal)) return 12;
                var reloaded = await Steam.Storage.BotDatabase.CreateOrLoad(database).ConfigureAwait(false);
                if (reloaded?.RefreshToken != "rotated-fixture-token") return 13;
            } finally { Directory.Delete(fixture, true); }
            await output.WriteLineAsync("Vault round-trip, actual database persistence, plaintext rejection and secret-absence tests passed.").ConfigureAwait(false);
            return 0;
        }
        if (args.Length != 3 || args[0] is not ("--bridge" or "--offline-test") || !int.TryParse(args[2], out int parentId)) return 2;
        OfflineTest = args[0] == "--offline-test";
        try {
            using var parent = Process.GetProcessById(parentId);
            parent.EnableRaisingEvents = true;
            parent.Exited += (_, _) => Environment.Exit(0);
            if (parent.HasExited) return 0;
            LootLazyVault.CheckPath(args[1]);
            Directory.CreateDirectory(args[1]);
            Directory.SetCurrentDirectory(Path.GetFullPath(args[1]));
            Directory.CreateDirectory("config");
            LootLazyVault.CheckPath(Path.Combine(Directory.GetCurrentDirectory(), "config"));
            LootLazyVault.CheckPath(Metadata);
            if (File.Exists(Metadata)) {
                if (new FileInfo(Metadata).Length > 262144) return 2;
                foreach (var account in JsonSerializer.Deserialize<Account[]>(File.ReadAllText(Metadata), JsonOptions) ?? []) {
                    if (!ValidId(account.Id) || account.Games.Length > 32 || Accounts.Count >= 20) return 2;
                    Accounts.Add(account.Id, account);
                    WriteConfig(account);
                }
            }
            // A cancelled/crashed uncommitted sign-in must not leave a saved account behind.
            foreach (string orphan in Directory.EnumerateFiles("config")) {
                string name = Path.GetFileName(orphan);
                string? ownedId = new[] { ".json.new", ".db.new", ".json", ".db" }.Where(suffix => name.EndsWith(suffix, StringComparison.Ordinal)).Select(suffix => name[..^suffix.Length]).FirstOrDefault(ValidId);
                if (ownedId == null || Accounts.ContainsKey(ownedId)) continue;
                LootLazyVault.CheckPath(orphan); File.Delete(orphan);
            }
            foreach (string path in new[] { "config/ASF.json", "config/ASF.db", "NLog.config" }) LootLazyVault.CheckPath(path);
            File.WriteAllText("config/ASF.json", "{\"Headless\":true,\"IPC\":false,\"AutoRestart\":false,\"UpdateChannel\":0,\"UpdatePeriod\":0,\"OptimizationMode\":1,\"PluginsUpdateList\":[],\"SteamOwnerID\":0}");
            File.WriteAllText("NLog.config", "<nlog xmlns=\"http://www.nlog-project.org/schemas/NLog.xsd\"><targets/><rules/></nlog>");
            if (!File.Exists("config/ASF.db")) File.WriteAllText("config/ASF.db", "{}");
            if (!OfflineTest) await Program.Init(["--path", Directory.GetCurrentDirectory(), "--no-restart", "--no-config-watch", "--no-config-migrate", "--no-steam-parental-generation"]).ConfigureAwait(false);
            if (!OfflineTest) {
                foreach (var account in Accounts.Values) await Bot.RegisterBot(account.Id).ConfigureAwait(false);
                foreach (var account in Accounts.Values) {
                    if (!account.RememberLogin || Bot.Bots?.TryGetValue(account.Id, out var bot) != true || !CanResume(account, bot)) continue;
                    Restoring[account.Id] = 1;
                    try { await bot.Start().ConfigureAwait(false); } catch { AuthErrors[account.Id] = "accounts_signin_required"; }
                }
            }
            using var cancellation = new CancellationTokenSource();
            var expiry = ExpireSessions(cancellation.Token);
            await output.WriteLineAsync(JsonSerializer.Serialize(new { ready = true, protocol = 1 })).ConfigureAwait(false);
            // Read bounded frames without exposing requests to ASF logging or an HTTP endpoint.
            var reader = Console.In;
            var frame = new StringBuilder();
            var buffer = new char[1];
            while (await reader.ReadAsync(buffer.AsMemory()).ConfigureAwait(false) > 0) {
                char character = buffer[0];
                if (character == '\r') continue;
                if (character != '\n') { if (frame.Length >= 16384) return 2; frame.Append(character); continue; }
                long id = 0;
                try {
                    using var document = JsonDocument.Parse(frame.ToString(), new JsonDocumentOptions { MaxDepth = 8 });
                    var request = document.RootElement;
                    id = request.GetProperty("id").GetInt64();
                    if (id < 1 || request.GetProperty("protocol").GetInt32() != 1) Fail("accounts_protocol");
                    string command = request.GetProperty("command").GetString() ?? "";
                    var body = request.TryGetProperty("body", out var requestBody) ? requestBody : default;
                    object result = await Handle(command, body).ConfigureAwait(false);
                    await output.WriteLineAsync(JsonSerializer.Serialize(new { id, ok = true, result }, JsonOptions)).ConfigureAwait(false);
                } catch (Exception error) {
                    string code = error is InvalidOperationException && error.Message.StartsWith("accounts_", StringComparison.Ordinal) ? error.Message : "accounts_failed";
                    await output.WriteLineAsync(JsonSerializer.Serialize(new { id, ok = false, error = code })).ConfigureAwait(false);
                } finally { frame.Clear(); }
            }
            await cancellation.CancelAsync().ConfigureAwait(false);
            try { await expiry.ConfigureAwait(false); } catch (OperationCanceledException) { }
            foreach (var challenge in Challenges.Values) challenge.Completion.TrySetResult(null);
            if (!OfflineTest) await Program.Shutdown().WaitAsync(TimeSpan.FromSeconds(5)).ConfigureAwait(false);
            return 0;
        } catch { return 2; }
    }
}
