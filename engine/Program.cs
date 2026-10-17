// LootLazy standalone Steam micro-client powered by SteamKit2.
// Account lifecycle implemented by LootLazy; Steam protocol provided by SteamKit2.
using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using Net.Codecrete.QrCodeGenerator;
using SteamKit2;
using SteamKit2.Authentication;
using SteamKit2.Internal;

namespace LootLazy.Steam;

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

internal static class SerializableFile {
    internal static async Task<bool> Write(string path, string content) {
        try {
            LootLazyVault.CheckPath(path);
            string encoded = LootLazyVault.Encode(path, content);
            string temp = path + ".new";
            LootLazyVault.CheckPath(temp);
            await File.WriteAllTextAsync(temp, encoded).ConfigureAwait(false);
            File.Move(temp, path, true);
            return true;
        } catch { return false; }
    }
}

internal sealed class BotDatabase {
    internal string? RefreshToken { get; set; }
    internal string? SteamGuardData { get; set; }
    private readonly string FilePath;

    internal BotDatabase(string path) => FilePath = path;

    internal static async Task<BotDatabase?> CreateOrLoad(string path) {
        LootLazyVault.CheckPath(path);
        var db = new BotDatabase(path);
        if (File.Exists(path)) {
            try {
                if (new FileInfo(path).Length > 262144) return null;
                string raw = await File.ReadAllTextAsync(path).ConfigureAwait(false);
                string decoded = LootLazyVault.Decode(path, raw);
                using var doc = JsonDocument.Parse(decoded);
                var root = doc.RootElement;
                if (root.TryGetProperty("BackingRefreshToken", out var brt)) db.RefreshToken = brt.GetString();
                else if (root.TryGetProperty("RefreshToken", out var rt)) db.RefreshToken = rt.GetString();
                if (root.TryGetProperty("BackingSteamGuardData", out var bsgd)) db.SteamGuardData = bsgd.GetString();
                else if (root.TryGetProperty("SteamGuardData", out var sgd)) db.SteamGuardData = sgd.GetString();
            } catch { return null; }
        }
        return db;
    }

    internal async Task StoreLogin(string refreshToken, string? guardData) {
        if (string.IsNullOrWhiteSpace(refreshToken)) throw new InvalidOperationException("accounts_login_failed");
        string? oldToken = RefreshToken, oldGuard = SteamGuardData;
        RefreshToken = refreshToken;
        SteamGuardData = guardData ?? oldGuard;
        try { await FlushLootLazyState().ConfigureAwait(false); }
        catch { RefreshToken = oldToken; SteamGuardData = oldGuard; throw; }
    }

    internal async Task FlushLootLazyState() {
        var payload = JsonSerializer.Serialize(new {
            BackingRefreshToken = RefreshToken,
            BackingSteamGuardData = SteamGuardData
        });
        if (!await SerializableFile.Write(FilePath, payload).ConfigureAwait(false)) throw new IOException("accounts_save_failed");
    }
}

internal enum EUserInputType {
    None,
    Login,
    Password,
    SteamGuard,
    TwoFactorAuthentication,
    QrCodeLogin
}

internal sealed class SteamAccount : IDisposable {
    internal readonly string Id;
    internal string Login;
    internal readonly SteamClient Client;
    internal readonly SteamUser User;
    internal readonly SteamFriends Friends;
    internal readonly SteamUnifiedMessages Unified;
    internal readonly CallbackManager Manager;
    internal readonly Player UnifiedPlayerService;

    internal BotDatabase? Database;
    internal volatile bool KeepRunning;
    internal bool IsConnectedAndLoggedOn => Client.IsConnected && User.SteamID != null;
    internal bool IsPlayingPossible => IsConnectedAndLoggedOn && !IsOccupied;
    internal volatile bool IsOccupied;
    internal SteamID? SteamID => User.SteamID;
    internal string SSteamID => SteamID?.ConvertToUInt64().ToString(CultureInfo.InvariantCulture) ?? "0";
    internal string? AvatarHash;
    internal EPersonaState DesiredPersona = EPersonaState.Online;
    internal EResult LastLogOnResult = EResult.Invalid;
    internal volatile bool Authenticating;
    internal volatile uint PlayingAppId;
    private readonly uint LoginId = NewLoginId();
    private int AuthenticationGeneration;
    private CancellationTokenSource? AuthenticationCancellation;
    private TaskCompletionSource ConnectionReady = NewSignal();
    private TaskCompletionSource? PendingDisconnect;
    private static TaskCompletionSource NewSignal() => new(TaskCreationOptions.RunContinuationsAsynchronously);
    internal static uint NewLoginId() {
        uint value;
        do { value = BitConverter.ToUInt32(RandomNumberGenerator.GetBytes(sizeof(uint))); } while (value == 0);
        return value;
    }

    private readonly Thread CallbackThread;
    private readonly CancellationTokenSource Disposed = new();

    internal bool HasLootLazyLogin => !string.IsNullOrEmpty(Database?.RefreshToken);

    internal SteamAccount(string id, string login, EPersonaState persona) {
        Id = id;
        Login = login;
        DesiredPersona = persona;
        Client = new SteamClient();
        Manager = new CallbackManager(Client);
        User = Client.GetHandler<SteamUser>()!;
        Friends = Client.GetHandler<SteamFriends>()!;
        Unified = Client.GetHandler<SteamUnifiedMessages>()!;
        UnifiedPlayerService = Unified.CreateService<Player>();

        Manager.Subscribe<SteamClient.ConnectedCallback>(OnConnected);
        Manager.Subscribe<SteamClient.DisconnectedCallback>(OnDisconnected);
        Manager.Subscribe<SteamUser.LoggedOnCallback>(OnLoggedOn);
        Manager.Subscribe<SteamUser.LoggedOffCallback>(OnLoggedOff);
        Manager.Subscribe<SteamFriends.PersonaStateCallback>(OnPersonaState);
        Manager.Subscribe<SteamUser.PlayingSessionStateCallback>(OnPlayingSessionState);

        CallbackThread = new Thread(() => {
            while (!Disposed.IsCancellationRequested) {
                try {
                    Manager.RunWaitCallbacks(TimeSpan.FromMilliseconds(100));
                } catch { }
            }
        }) { IsBackground = true };
        CallbackThread.Start();
    }

    private void LogOnInternal() {
        if (!KeepRunning || Authenticating) return;
        if (string.IsNullOrEmpty(Database?.RefreshToken)) {
            Program.AuthenticationFailed(this, "accounts_signin_required");
            Stop();
            return;
        }
        User.LogOn(new SteamUser.LogOnDetails {
            Username = Login, AccessToken = Database.RefreshToken,
            ShouldRememberPassword = true, LoginID = LoginId
        });
    }

    private void OnConnected(SteamClient.ConnectedCallback callback) {
        bool authenticationOwnsConnection = Authenticating;
        ConnectionReady.TrySetResult();
        if (!authenticationOwnsConnection) LogOnInternal();
    }

    private void OnDisconnected(SteamClient.DisconnectedCallback callback) {
        var expected = Interlocked.Exchange(ref PendingDisconnect, null);
        ConnectionReady.TrySetException(new IOException("accounts_connection_failed"));
        ConnectionReady = NewSignal();
        if (expected != null) { expected.TrySetResult(); return; }
        Program.StopRequested(this);
        if (Authenticating) {
            try { AuthenticationCancellation?.Cancel(); } catch (ObjectDisposedException) { }
            return;
        }
        if (!KeepRunning) return;
        // Reconnect transport loss only. Session conflicts and authentication failures stop the account.
        _ = Task.Run(async () => {
            try {
                await Task.Delay(3000, Disposed.Token).ConfigureAwait(false);
                if (KeepRunning && !Client.IsConnected) Client.Connect();
            } catch (OperationCanceledException) { }
        });
    }

    private async void OnLoggedOn(SteamUser.LoggedOnCallback callback) {
        try {
            if (!KeepRunning) { Client.Disconnect(); return; }
            LastLogOnResult = callback.Result;
            Program.LoginResult(this, callback.Result);
            if (callback.Result == EResult.OK) {
                Friends.SetPersonaState(DesiredPersona);
                await Program.ResetGames(this).ConfigureAwait(false);
            } else {
                if (callback.Result is not (EResult.LoggedInElsewhere or EResult.AlreadyLoggedInElsewhere or EResult.LogonSessionReplaced))
                    Program.AuthenticationFailed(this, Program.LoginError(callback.Result) ?? "accounts_signin_required");
                Stop();
            }
        } catch { Program.AuthenticationFailed(this, "accounts_connection_failed"); Stop(); }
    }

    private void OnLoggedOff(SteamUser.LoggedOffCallback callback) {
        LastLogOnResult = callback.Result;
        Program.LoginResult(this, callback.Result);
        if (callback.Result is EResult.LoggedInElsewhere or EResult.AlreadyLoggedInElsewhere or EResult.LogonSessionReplaced) {
            Program.SessionConflict(this);
            Stop();
        } else if (callback.Result != EResult.OK) {
            Program.AuthenticationFailed(this, "accounts_connection_failed");
            Stop();
        }
    }

    internal void ApplyPlayingState(bool blocked, uint appId) {
        IsOccupied = blocked;
        PlayingAppId = blocked ? appId : 0;
        if (blocked) Program.Occupied(this);
    }

    private void OnPlayingSessionState(SteamUser.PlayingSessionStateCallback callback) =>
        ApplyPlayingState(callback.PlayingBlocked, callback.PlayingAppID);

    internal sealed class GuardAuthenticator(SteamAccount account, CancellationToken token) : IAuthenticator {
        public async Task<string> GetDeviceCodeAsync(bool previousCodeWasIncorrect) =>
            await ReadCode(EUserInputType.TwoFactorAuthentication, previousCodeWasIncorrect).ConfigureAwait(false);
        public async Task<string> GetEmailCodeAsync(string email, bool previousCodeWasIncorrect) =>
            await ReadCode(EUserInputType.SteamGuard, previousCodeWasIncorrect).ConfigureAwait(false);
        private async Task<string> ReadCode(EUserInputType type, bool incorrect) {
            if (incorrect) Program.AuthenticationFailed(account, "accounts_guard_invalid");
            string? value = await Program.InputAsync(type, account.Id, token).ConfigureAwait(false);
            return value ?? throw new OperationCanceledException(token);
        }
        public Task<bool> AcceptDeviceConfirmationAsync() => Task.FromResult(false);
    }

    internal void BeginSignIn(string password, bool qr) {
        if (!qr && string.IsNullOrEmpty(password)) { Start(); return; }
        Authenticating = true;
        KeepRunning = true;
        int generation = Interlocked.Increment(ref AuthenticationGeneration);
        var flow = Program.BeginConnection(this);
        var cancellation = CancellationTokenSource.CreateLinkedTokenSource(flow.Token, Disposed.Token);
        cancellation.CancelAfter(TimeSpan.FromMinutes(3));
        AuthenticationCancellation = cancellation;
        string? attempt = Program.QrAttemptId(this);
        _ = Task.Run(async () => {
            using (flow)
            using (cancellation) {
                try {
                    var token = cancellation.Token;
                    var disconnect = PendingDisconnect;
                    if (disconnect != null) await disconnect.Task.WaitAsync(token).ConfigureAwait(false);
                    token.ThrowIfCancellationRequested();
                    if (!Client.IsConnected) {
                        Client.Connect();
                        await ConnectionReady.Task.WaitAsync(token).ConfigureAwait(false);
                    }
                    var details = new AuthSessionDetails {
                        DeviceFriendlyName = "LootLazy", Username = Login, Password = password,
                        GuardData = Database?.SteamGuardData, IsPersistentSession = true,
                        Authenticator = new GuardAuthenticator(this, token)
                    };
                    AuthPollResult result;
                    if (qr) {
                        var auth = await Client.Authentication.BeginAuthSessionViaQRAsync(details).WaitAsync(token).ConfigureAwait(false);
                        Program.PublishQr(this, attempt, auth.ChallengeURL);
                        auth.ChallengeURLChanged = () => Program.PublishQr(this, attempt, auth.ChallengeURL);
                        result = await auth.PollingWaitForResultAsync(token).ConfigureAwait(false);
                    } else {
                        var auth = await Client.Authentication.BeginAuthSessionViaCredentialsAsync(details).WaitAsync(token).ConfigureAwait(false);
                        details.Password = null;
                        password = "";
                        result = await auth.PollingWaitForResultAsync(token).ConfigureAwait(false);
                    }
                    token.ThrowIfCancellationRequested();
                    if (generation != AuthenticationGeneration || !KeepRunning) return;
                    if (!string.IsNullOrEmpty(Login) && !result.AccountName.Equals(Login, StringComparison.OrdinalIgnoreCase)) {
                        throw new InvalidOperationException("accounts_qr_mismatch");
                    }
                    Database ??= new BotDatabase(Path.Combine("config", Id + ".db"));
                    try { await Database.StoreLogin(result.RefreshToken, result.NewGuardData).ConfigureAwait(false); }
                    catch (IOException) { throw new InvalidOperationException("accounts_save_failed"); }
                    token.ThrowIfCancellationRequested();
                    if (generation != AuthenticationGeneration || !KeepRunning) return;
                    Login = result.AccountName;
                    Program.ClearQr(this, attempt);
                    Authenticating = false;
                    LogOnInternal();
                } catch (OperationCanceledException) {
                    if (generation == AuthenticationGeneration && KeepRunning) {
                        Program.AuthenticationFailed(this, qr ? "accounts_qr_expired" : "accounts_connection_failed");
                        Stop();
                    }
                } catch (Exception error) {
                    if (generation == AuthenticationGeneration) {
                        string code = error is AuthenticationException authError ? Program.LoginError(authError.Result) ?? "accounts_login_failed"
                            : error is InvalidOperationException && error.Message is "accounts_qr_mismatch" or "accounts_save_failed" ? error.Message
                            : qr ? "accounts_qr_failed" : "accounts_connection_failed";
                        Program.AuthenticationFailed(this, code);
                        Stop();
                    }
                } finally {
                    password = "";
                    if (generation == AuthenticationGeneration) Authenticating = false;
                    Interlocked.CompareExchange(ref AuthenticationCancellation, null, cancellation);
                }
            }
        });
    }

    private void OnPersonaState(SteamFriends.PersonaStateCallback callback) {
        if (callback.FriendID == User.SteamID) {
            if (callback.AvatarHash != null && callback.AvatarHash.Length > 0) {
                AvatarHash = Convert.ToHexString(callback.AvatarHash).ToLowerInvariant();
            }
            // Persona is display data. Only PlayingSessionState authorizes sending games.
        }
    }

    internal void PlayGames(IReadOnlyCollection<uint> gameIDs) {
        if (!IsConnectedAndLoggedOn) { if (gameIDs.Count == 0) return; throw new InvalidOperationException("accounts_not_ready"); }
        if (gameIDs.Count > 0 && IsOccupied) throw new InvalidOperationException("accounts_playing_elsewhere");
        var request = new ClientMsgProtobuf<CMsgClientGamesPlayed>(EMsg.ClientGamesPlayedWithDataBlob);
        foreach (uint gameID in gameIDs.Where(g => g > 0).Distinct()) {
            request.Body.games_played.Add(new CMsgClientGamesPlayed.GamePlayed {
                game_id = new GameID(gameID).ToUInt64()
            });
        }
        Client.Send(request);
    }

    internal async Task<Dictionary<uint, string>?> GetOwnedGames(ulong steamId) {
        if (!IsConnectedAndLoggedOn) return null;
        var request = new CPlayer_GetOwnedGames_Request {
            steamid = steamId,
            include_appinfo = true,
            include_free_sub = true,
            include_played_free_games = true,
            skip_unvetted_apps = false
        };
        var response = await UnifiedPlayerService.GetOwnedGames(request).ToTask().ConfigureAwait(false);
        if (response.Result != EResult.OK) return null;
        return response.Body.games.Where(g => g.appid > 0 && !string.IsNullOrWhiteSpace(g.name)).ToDictionary(g => (uint)g.appid, g => g.name);
    }

    internal async Task FlushLootLazyLogin() {
        if (Database != null) {
            await Database.FlushLootLazyState().ConfigureAwait(false);
        }
    }

    internal void ResetPersonaState() {
        if (IsConnectedAndLoggedOn) {
            Friends.SetPersonaState(DesiredPersona);
        }
    }

    internal void Start() {
        KeepRunning = true;
        var disconnect = PendingDisconnect;
        if (disconnect != null) {
            int generation = AuthenticationGeneration;
            _ = Task.Run(async () => {
                try {
                    await disconnect.Task.WaitAsync(TimeSpan.FromSeconds(15), Disposed.Token).ConfigureAwait(false);
                    if (KeepRunning && generation == AuthenticationGeneration) Start();
                } catch (OperationCanceledException) { }
                catch { if (generation == AuthenticationGeneration) { Program.AuthenticationFailed(this, "accounts_connection_failed"); Stop(); } }
            });
            return;
        }
        if (!Client.IsConnected) {
            Client.Connect();
        } else if (!IsConnectedAndLoggedOn) {
            LogOnInternal();
        }
    }

    internal void Stop(bool force = true) {
        KeepRunning = false;
        Interlocked.Increment(ref AuthenticationGeneration);
        var cancellation = Interlocked.Exchange(ref AuthenticationCancellation, null);
        try { cancellation?.Cancel(); } catch (ObjectDisposedException) { }
        Authenticating = false;
        Program.StopRequested(this);
        if (Client.IsConnected) {
            try { PlayGames(Array.Empty<uint>()); } catch { }
            Interlocked.CompareExchange(ref PendingDisconnect, NewSignal(), null);
            Client.Disconnect();
        }
    }

    public void Dispose() {
        Stop(true);
        Disposed.Cancel();
        // Let the callback loop observe cancellation before disposing its source.
        if (Thread.CurrentThread != CallbackThread) CallbackThread.Join(TimeSpan.FromSeconds(1));
    }
}

internal static class Program {
    internal static bool Enabled => true;

    // Self-test only: inspect anonymous serialized status packets in memory, without connecting or logging in.
    private sealed class PresencePacketFixture : IDebugNetworkListener {
        internal readonly List<uint> States = [];
        internal bool Invalid;
        public void OnIncomingNetworkMessage(EMsg type, byte[] data) { }
        public void OnOutgoingNetworkMessage(EMsg type, byte[] data) {
            try {
                if (type != EMsg.ClientChangeStatus) { Invalid = true; return; }
                var packet = new PacketClientMsgProtobuf(type, data);
                var message = new ClientMsgProtobuf<CMsgClientChangeStatus>(packet);
                States.Add(message.Body.persona_state);
            } catch { Invalid = true; }
        }
    }

    private sealed record Account(string Id, string Label, string Login, uint[] Games, string Presence = "online", bool RememberLogin = true, string? AvatarHash = null);

    private static EPersonaState Persona(string presence) => presence switch {
        "online" => EPersonaState.Online, "away" => EPersonaState.Away,
        "invisible" => EPersonaState.Invisible, "offline" => EPersonaState.Offline,
        _ => throw new InvalidOperationException("accounts_invalid")
    };

    private sealed record Challenge(string Id, EUserInputType Type, TaskCompletionSource<string?> Completion);

    internal sealed class Session {
        internal uint[] Games = [];
        internal Dictionary<uint, DateTimeOffset> GameStarts = [];
        internal DateTimeOffset Started;
        internal int Minutes;
        internal bool Blocked;
    }

    private static readonly ConcurrentDictionary<string, DateTimeOffset> QrLogins = new(StringComparer.Ordinal);
    internal static bool WantsQrLogin(SteamAccount account) => QrLogins.TryGetValue(account.Id, out var deadline) && deadline > DateTimeOffset.UtcNow;
    private static bool ValidLogin(string login) => login.Length is > 0 and <= 64 && login.All(c => char.IsAsciiLetterOrDigit(c) || c == '_');

    private static string[] EncodeQr(string payload) {
        if (!Uri.TryCreate(payload, UriKind.Absolute, out var uri)) throw new InvalidOperationException("accounts_invalid");
        if (uri.Scheme != "https" || uri.Host != "s.team" || uri.Port != 443 || !string.IsNullOrEmpty(uri.UserInfo) || !uri.AbsolutePath.StartsWith("/q/", StringComparison.Ordinal) || payload.Length > 512) throw new InvalidOperationException("accounts_invalid");
        var code = QrCode.EncodeText(payload, QrCode.Ecc.Medium);
        if (code.Size > 97) throw new InvalidOperationException("accounts_invalid");
        return Enumerable.Range(0, code.Size).Select(y => new string(Enumerable.Range(0, code.Size).Select(x => code.GetModule(x, y) ? '1' : '0').ToArray())).ToArray();
    }

    // Only the protocol thread persists identities; callbacks never mutate account metadata.
    private static async Task BindQrIdentities() {
        foreach (var entry in Accounts.Values.ToArray()) {
            if (!AccountsMap.TryGetValue(entry.Id, out var account) || !account.IsConnectedAndLoggedOn || !QrLogins.ContainsKey(entry.Id)) continue;
            string login = account.Login ?? "";
            if (!ValidLogin(login) || Accounts.Values.Any(other => other.Id != entry.Id && other.Login.Equals(login, StringComparison.OrdinalIgnoreCase))) {
                AuthErrors[entry.Id] = "accounts_duplicate";
                QrLogins.TryRemove(entry.Id, out _);
                account.Stop(true);
                continue;
            }
            if (entry.Login != login) {
                var updated = entry with { Login = login };
                WriteConfig(updated);
                Accounts[entry.Id] = updated;
                try { Save(); } catch { Accounts[entry.Id] = entry; throw; }
            }
            QrLogins.TryRemove(entry.Id, out _);
        }
        foreach (string id in Restoring.Keys) {
            if (!Accounts.TryGetValue(id, out var entry) || !AccountsMap.TryGetValue(id, out var account) || !account.IsConnectedAndLoggedOn) continue;
            await account.FlushLootLazyLogin().ConfigureAwait(false);
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
        internal ConnectionFlow(string id, string? attempt) {
            Owner = ConnectionFlows.GetOrAdd(id, _ => new(StringComparer.Ordinal));
            QrAttempt = attempt;
            Owner[Id] = this;
        }
        internal void Cancel() { try { Cancellation.Cancel(); } catch (ObjectDisposedException) { } }
        public void Dispose() { Owner.TryRemove(Id, out _); Cancellation.Dispose(); Completion.TrySetResult(); }
    }

    internal static ConnectionFlow BeginConnection(SteamAccount account) => new(account.Id, QrAttemptId(account));
    private static void CancelConnections(string id) { if (ConnectionFlows.TryGetValue(id, out var flows)) foreach (var flow in flows.Values) flow.Cancel(); }
    private static async Task WaitConnections(string id) {
        if (!ConnectionFlows.TryGetValue(id, out var flows)) return;
        try { await Task.WhenAll(flows.Values.Select(flow => flow.Completion.Task)).WaitAsync(TimeSpan.FromSeconds(12)).ConfigureAwait(false); }
        catch (TimeoutException) { Fail("accounts_auth_busy"); }
    }

    private static readonly ConcurrentDictionary<string, byte> Restoring = new(StringComparer.Ordinal);
    private static bool CanResume(Account account, SteamAccount? client) => !Drafts.Contains(account.Id) && client?.HasLootLazyLogin == true && (!AuthErrors.TryGetValue(account.Id, out var error) || error == "accounts_session_conflict");
    private static readonly HashSet<string> Drafts = new(StringComparer.Ordinal);
    internal static string? QrAttemptId(SteamAccount account) => QrLogins.TryGetValue(account.Id, out var deadline) ? deadline.Ticks.ToString(CultureInfo.InvariantCulture) : null;

    internal static void PublishQr(SteamAccount account, string? attempt, string payload) {
        if (attempt == null || attempt != QrAttemptId(account) || !WantsQrLogin(account)) return;
        QrCodes[account.Id] = new QrChallenge(attempt, payload, Guid.NewGuid().ToString("N"));
    }

    internal static void ClearQr(SteamAccount account, string? attempt) {
        if (QrCodes.TryGetValue(account.Id, out var qr) && qr.Attempt == attempt) ((ICollection<KeyValuePair<string, QrChallenge>>)QrCodes).Remove(new(account.Id, qr));
    }

    internal static void QrMismatch(SteamAccount account) => AuthErrors[account.Id] = "accounts_qr_mismatch";

    internal static bool QrFailed(SteamAccount account, string? attempt) {
        if (attempt == null || QrAttemptId(account) != attempt) return false;
        AuthErrors.TryAdd(account.Id, "accounts_qr_failed");
        QrLogins.TryRemove(account.Id, out _);
        QrCodes.TryRemove(account.Id, out _);
        return true;
    }

    private static bool IsBusy() => Drafts.Count > 0 || Challenges.Count > 0 || Sessions.Values.Any(session => session.Games.Length > 0) || AccountsMap.Values.Any(a => a.KeepRunning && !a.IsConnectedAndLoggedOn);

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
            Games = games.ToArray(),
            Started = previous.Started,
            Minutes = previous.Minutes,
            Blocked = previous.Blocked,
            GameStarts = games.ToDictionary(game => game, game => previous.GameStarts.TryGetValue(game, out var started) ? started : previous.Games.Contains(game) ? previous.Started : now)
        };
        if (!Sessions.TryUpdate(id, updated, previous)) return;
        try { await send().ConfigureAwait(false); }
        catch { Sessions.TryUpdate(id, previous, updated); throw; }
        if (games.Length == 0) ((ICollection<KeyValuePair<string, Session>>)Sessions).Remove(new(id, updated));
    }

    private static readonly Dictionary<string, Account> Accounts = new(StringComparer.Ordinal);
    internal static readonly ConcurrentDictionary<string, SteamAccount> AccountsMap = new(StringComparer.Ordinal);
    private static readonly ConcurrentDictionary<string, Challenge> Challenges = new(StringComparer.Ordinal);
    private sealed record Library(ulong SteamId, Dictionary<uint, string> Games);
    private static readonly ConcurrentDictionary<string, Library> Libraries = new(StringComparer.Ordinal);
    private static readonly ConcurrentDictionary<string, string> LibraryRequests = new(StringComparer.Ordinal);
    private static readonly ConcurrentDictionary<string, byte> LibraryErrors = new(StringComparer.Ordinal);
    internal static readonly ConcurrentDictionary<string, Session> Sessions = new(StringComparer.Ordinal);
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

    internal static async Task<string?> InputAsync(EUserInputType type, string id, CancellationToken token = default) {
        if (!ValidId(id) || !AccountsMap.TryGetValue(id, out var inputAccount) || !inputAccount.KeepRunning) return null;
        if (Restoring.ContainsKey(id)) { AuthErrors[id] = "accounts_signin_required"; return null; }
        if (type == EUserInputType.QrCodeLogin) return WantsQrLogin(inputAccount) ? "Y" : "N";
        var challenge = new Challenge(Guid.NewGuid().ToString("N"), type, new(TaskCreationOptions.RunContinuationsAsynchronously));
        if (Challenges.TryRemove(id, out var old)) old.Completion.TrySetResult(null);
        Challenges[id] = challenge;
        try { return await challenge.Completion.Task.WaitAsync(TimeSpan.FromMinutes(3), token).ConfigureAwait(false); }
        catch (TimeoutException) { return null; }
        finally { if (Challenges.TryGetValue(id, out var current) && current.Id == challenge.Id) Challenges.TryRemove(id, out _); }
    }

    internal static void AuthenticationFailed(SteamAccount account, string error) => AuthErrors[account.Id] = error;

    internal static string? LoginError(EResult result) => result switch {
        EResult.InvalidPassword or EResult.AccountNotFound => "accounts_credentials_invalid",
        EResult.InvalidLoginAuthCode or EResult.TwoFactorCodeMismatch or EResult.ExpiredLoginAuthCode => "accounts_guard_invalid",
        EResult.Timeout or EResult.NoConnection or EResult.ServiceUnavailable => "accounts_connection_failed",
        _ => null
    };

    private static readonly ConcurrentDictionary<string, DateTimeOffset> LoginReadyAt = new(StringComparer.Ordinal);
    internal static TimeSpan LoginSettleDelay(DateTimeOffset loggedOn, DateTimeOffset now) {
        var remaining = TimeSpan.FromSeconds(2) - (now - loggedOn);
        return remaining > TimeSpan.FromSeconds(2) ? TimeSpan.FromSeconds(2) : remaining > TimeSpan.Zero ? remaining : TimeSpan.Zero;
    }

    internal static bool LoginResult(SteamAccount account, EResult result) {
        if (result is EResult.LoggedInElsewhere or EResult.AlreadyLoggedInElsewhere or EResult.LogonSessionReplaced) { SessionConflict(account); return true; }
        if (result == EResult.OK) { AuthErrors.TryRemove(account.Id, out _); LoginReadyAt[account.Id] = DateTimeOffset.UtcNow; }
        else if (LoginError(result) is { } error) AuthErrors[account.Id] = error;
        return result is EResult.InvalidPassword or EResult.AccountNotFound;
    }

    internal static void SessionConflict(SteamAccount account) => SessionConflict(account.Id);
    internal static void SessionConflict(string id) {
        if (Sessions.TryGetValue(id, out var session)) { session.Games = []; session.Blocked = true; }
        AuthErrors[id] = "accounts_session_conflict";
    }

    internal static void StopRequested(SteamAccount account) {
        LoginReadyAt.TryRemove(account.Id, out _);
        if (Sessions.TryGetValue(account.Id, out var session)) session.Games = [];
    }

    internal static void Occupied(SteamAccount account) {
        if (Sessions.TryGetValue(account.Id, out var session)) { session.Games = []; session.Blocked = true; }
    }

    internal static async Task ResetGames(SteamAccount account) {
        if (!account.IsConnectedAndLoggedOn) return;
        await PlayLock.WaitAsync().ConfigureAwait(false);
        try {
            if (LoginReadyAt.TryGetValue(account.Id, out var loggedOn) && Sessions.TryGetValue(account.Id, out var pending) && pending.Games.Length > 0) {
                var delay = LoginSettleDelay(loggedOn, DateTimeOffset.UtcNow);
                if (delay > TimeSpan.Zero) await Task.Delay(delay).ConfigureAwait(false);
                if (!LoginReadyAt.TryGetValue(account.Id, out var currentLogon) || currentLogon != loggedOn) return;
            }
            if (!account.IsConnectedAndLoggedOn) { StopRequested(account); return; }
            uint[] games = Sessions.TryGetValue(account.Id, out var session) ? session.Games : [];
            if (!account.IsPlayingPossible) { Occupied(account); return; }
            account.PlayGames(games);
        } finally { PlayLock.Release(); }
    }

    private static bool ValidAvatar(string? hash) => hash?.Length == 40 && hash.All(Uri.IsHexDigit);

    private static void RefreshAvatars() {
        var changed = new Dictionary<string, Account>();
        foreach (var account in Accounts.Values.ToArray()) {
            if (!AccountsMap.TryGetValue(account.Id, out var client) || !client.IsConnectedAndLoggedOn || !ValidAvatar(client.AvatarHash) || account.AvatarHash == client.AvatarHash) continue;
            changed[account.Id] = account;
            Accounts[account.Id] = account with { AvatarHash = client.AvatarHash };
        }
        if (changed.Count == 0) return;
        try { Save(); } catch { foreach (var entry in changed) Accounts[entry.Key] = entry.Value; }
    }

    private static object Snapshot() {
        var result = Accounts.Values.Select(account => {
            AccountsMap.TryGetValue(account.Id, out var client);
            Challenges.TryGetValue(account.Id, out var input);
            Sessions.TryGetValue(account.Id, out var session);
            bool connected = client?.IsConnectedAndLoggedOn == true;
            bool running = connected && client!.IsPlayingPossible && session?.Games.Length > 0;
            string state = input != null ? "input" : running ? "running" : connected && !client!.IsPlayingPossible ? "blocked" : connected ? "ready" : client?.KeepRunning == true ? "connecting" : "offline";
            string? avatarHash = ValidAvatar(client?.AvatarHash) ? client!.AvatarHash : ValidAvatar(account.AvatarHash) ? account.AvatarHash : null;
            uint playingAppId = connected && client!.IsOccupied ? client.PlayingAppId : 0;
            string? playingGameName = playingAppId > 0 && client!.SteamID != null ? client.Friends.GetFriendGamePlayedName(client.SteamID) : null;
            return new {
                avatarUrl = avatarHash == null ? null : "https://avatars.steamstatic.com/" + avatarHash.ToLowerInvariant() + "_medium.jpg",
                playingAppId,
                playingGameName,
                runningGameCount = running ? session!.Games.Length : 0,
                id = account.Id,
                label = account.Label,
                login = account.Login,
                draft = Drafts.Contains(account.Id),
                games = account.Games,
                presence = account.Presence,
                rememberLogin = account.RememberLogin,
                canResume = CanResume(account, client),
                state,
                connected,
                active = client?.KeepRunning == true,
                steamId = client?.SSteamID ?? "0",
                input = input == null ? null : new { id = input.Id, type = input.Type.ToString() },
                elapsedSeconds = running ? Math.Max(0, (long)(DateTimeOffset.UtcNow - session!.Started).TotalSeconds) : 0,
                minutes = session?.Minutes ?? 0,
                gameElapsedSeconds = running ? session!.Games.ToDictionary(game => game, game => Math.Max(0, (long)(DateTimeOffset.UtcNow - (session.GameStarts.TryGetValue(game, out var started) ? started : session.Started)).TotalSeconds)) : new Dictionary<uint, long>(),
                error = AuthErrors.TryGetValue(account.Id, out var authError) ? authError : client == null && !OfflineTest ? "accounts_restore_failed" : client != null && !client.KeepRunning && client.LastLogOnResult is not (EResult.Invalid or EResult.OK) ? "accounts_login_failed" : null
            };
        }).ToArray();
        return new { protocol = 1, accounts = result, active = IsBusy() };
    }

    private static Account Find(JsonElement body) {
        string id = Text(body, "id");
        if (!ValidId(id) || !Accounts.TryGetValue(id, out var account)) { Fail("accounts_missing"); return null!; }
        return account;
    }

    private static object LibraryResult(SteamAccount client, JsonElement body) {
        string id = client.Id, query = Text(body, "query").Trim();
        if (query.Length > 160) { Fail("accounts_invalid"); return null!; }
        if (body.TryGetProperty("refresh", out var refresh) && refresh.ValueKind == JsonValueKind.True && !LibraryRequests.ContainsKey(id)) {
            Libraries.TryRemove(id, out _);
            LibraryErrors.TryRemove(id, out _);
        }
        HashSet<uint>? requested = null;
        if (body.TryGetProperty("appIds", out var ids)) {
            if (ids.ValueKind != JsonValueKind.Array || ids.GetArrayLength() > 30) Fail("accounts_invalid");
            requested = new HashSet<uint>();
            foreach (var value in ids.EnumerateArray()) {
                if (value.ValueKind != JsonValueKind.Number || !value.TryGetUInt32(out uint game) || game == 0) Fail("accounts_invalid");
                requested.Add(value.GetUInt32());
            }
        }
        if (Libraries.TryGetValue(id, out var library) && client.SteamID != null && library.SteamId == client.SteamID.ConvertToUInt64()) {
            var games = library.Games.Where(game => requested != null ? requested.Contains(game.Key) : query.Length == 0 || game.Value.Contains(query, StringComparison.OrdinalIgnoreCase) || game.Key.ToString(CultureInfo.InvariantCulture).Contains(query, StringComparison.Ordinal)).OrderBy(game => game.Value, StringComparer.OrdinalIgnoreCase).Skip(body.TryGetProperty("offset", out var offset) && offset.ValueKind == JsonValueKind.Number && offset.TryGetInt32(out var page) ? Math.Clamp(page, 0, 100000) : 0).Take(50).Select(game => new { id = game.Key, name = game.Value[..Math.Min(game.Value.Length, 160)] }).ToArray();
            return new { games, total = library.Games.Count, loading = false, active = IsBusy(), error = (string?)null };
        }
        if (!LibraryErrors.ContainsKey(id)) {
            string nonce = Guid.NewGuid().ToString("N");
            if (LibraryRequests.TryAdd(id, nonce)) {
                ulong steamId = client.SteamID?.ConvertToUInt64() ?? 0;
                _ = Task.Run(async () => {
                    try {
                        var games = await client.GetOwnedGames(steamId).WaitAsync(TimeSpan.FromSeconds(15)).ConfigureAwait(false);
                        if (!LibraryRequests.TryGetValue(id, out var current) || current != nonce) return;
                        if (games == null || !client.IsConnectedAndLoggedOn || client.SteamID?.ConvertToUInt64() != steamId) { LibraryErrors[id] = 1; return; }
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
            if (!OfflineTest) {
                var client = new SteamAccount(account.Id, account.Login, Persona(account.Presence));
                client.Database = await BotDatabase.CreateOrLoad(Path.Combine("config", account.Id + ".db")).ConfigureAwait(false);
                AccountsMap[account.Id] = client;
            }
            Accounts.Add(account.Id, account);
            if (body.TryGetProperty("draft", out var draft) && draft.ValueKind == JsonValueKind.True) Drafts.Add(account.Id);
            Save();
            return Snapshot();
        }
        var entry = Find(body);
        AccountsMap.TryGetValue(entry.Id, out var selected);
        if (command == "remove") {
            QrLogins.TryRemove(entry.Id, out _); QrCodes.TryRemove(entry.Id, out _); AuthErrors.TryRemove(entry.Id, out _);
            if (Challenges.TryRemove(entry.Id, out var pending)) pending.Completion.TrySetResult(null);
            Restoring.TryRemove(entry.Id, out _);
            Sessions.TryRemove(entry.Id, out _);
            Libraries.TryRemove(entry.Id, out _); LibraryRequests.TryRemove(entry.Id, out _); LibraryErrors.TryRemove(entry.Id, out _);
            if (selected != null) {
                CancelConnections(entry.Id);
                selected.Stop(true);
                await WaitConnections(entry.Id).ConfigureAwait(false);
                selected.Dispose();
                AccountsMap.TryRemove(entry.Id, out _);
            }
            foreach (string suffix in new[] { ".json", ".db", ".db.new", ".json.new" }) {
                string ownedFile = Path.Combine("config", entry.Id + suffix);
                try {
                    LootLazyVault.CheckPath(ownedFile);
                    if (File.Exists(ownedFile)) File.Delete(ownedFile);
                } catch { }
            }
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
            if (!OfflineTest && selected?.IsConnectedAndLoggedOn != true) Fail("accounts_not_ready");
            if (Sessions.TryGetValue(entry.Id, out var playing) && playing.Games.Length > 0 && selected?.IsPlayingPossible != true) Fail("accounts_playing_elsewhere");
            Accounts[entry.Id] = entry with { Games = games };
            try { Save(); } catch { Accounts[entry.Id] = entry; throw; }
            try { await UpdateSessionGames(entry.Id, games, () => { selected?.PlayGames(games); return Task.CompletedTask; }).ConfigureAwait(false); }
            catch { Accounts[entry.Id] = entry; Save(); throw; }
            return Snapshot();
        }
        if (command == "presence") {
            string presence = Text(body, "presence");
            var persona = Persona(presence);
            Accounts[entry.Id] = entry with { Presence = presence };
            try { Save(); } catch { Accounts[entry.Id] = entry; throw; }
            if (selected != null) {
                selected.DesiredPersona = persona;
                selected.ResetPersonaState();
            }
            return Snapshot();
        }
        if (!OfflineTest && selected == null && command == "signin") {
            // Preserve the unreadable database until a new login is successfully persisted.
            selected = new SteamAccount(entry.Id, entry.Login, Persona(entry.Presence)) {
                Database = new BotDatabase(Path.Combine("config", entry.Id + ".db"))
            };
            AccountsMap[entry.Id] = selected;
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
            AuthErrors.TryRemove(entry.Id, out _);
            Restoring[entry.Id] = 1;
            selected.DesiredPersona = Persona(entry.Presence);
            selected.Start();
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
            selected.Login = qr && Drafts.Contains(entry.Id) ? "" : entry.Login;
            selected.DesiredPersona = Persona(entry.Presence);
            selected.BeginSignIn(password, qr);
        } else if (command == "input") {
            if (!Challenges.TryGetValue(entry.Id, out var challenge) || challenge.Id != Text(body, "challengeId") || challenge.Completion.Task.IsCompleted) Fail("accounts_input_expired");
            string value = Text(body, "value");
            if (value.Length is < 1 or > 256) Fail("accounts_invalid");
            challenge!.Completion.TrySetResult(value);
        } else if (command == "start") {
            if (!selected.IsConnectedAndLoggedOn) Fail("accounts_not_ready");
            if (!selected.IsPlayingPossible) Fail("accounts_playing_elsewhere");
            if (entry.Games.Length == 0) Fail("accounts_choose_games");
            if (body.TryGetProperty("minutes", out var legacyMinutes) && (legacyMinutes.ValueKind != JsonValueKind.Number || !legacyMinutes.TryGetInt32(out var oldMinutes) || oldMinutes < 0 || oldMinutes > 1440)) Fail("accounts_invalid");
            const int minutes = 0;
            if (Sessions.TryGetValue(entry.Id, out var activeSession) && activeSession.Games.Length > 0) return Snapshot();
            var started = DateTimeOffset.UtcNow;
            Sessions[entry.Id] = new Session { Games = entry.Games.ToArray(), Started = started, Minutes = minutes, GameStarts = entry.Games.ToDictionary(game => game, _ => started) };
            try {
                await ResetGames(selected).ConfigureAwait(false);
                if (!selected.IsConnectedAndLoggedOn) Fail("accounts_not_ready");
                if (!selected.IsPlayingPossible) Fail("accounts_playing_elsewhere");
            } catch { Sessions.TryRemove(entry.Id, out _); throw; }
        } else if (command == "stop") {
            try { await EndSession(entry.Id, () => { selected.PlayGames([]); return Task.CompletedTask; }).ConfigureAwait(false); }
            catch { selected.Stop(true); throw; }
        } else if (command == "disconnect") {
            if (body.TryGetProperty("signOut", out var signOut) && signOut.ValueKind == JsonValueKind.True) {
                Accounts[entry.Id] = entry with { RememberLogin = false };
                try { Save(); } catch { Accounts[entry.Id] = entry; throw; }
            }
            Restoring.TryRemove(entry.Id, out _);
            CancelConnections(entry.Id);
            QrLogins.TryRemove(entry.Id, out _); QrCodes.TryRemove(entry.Id, out _);
            if (Challenges.TryRemove(entry.Id, out var pending)) pending.Completion.TrySetResult(null);
            try { await EndSession(entry.Id, () => { selected.PlayGames([]); return Task.CompletedTask; }).ConfigureAwait(false); }
            finally { selected.Stop(true); await WaitConnections(entry.Id).ConfigureAwait(false); }
        } else { Fail("accounts_invalid"); }
        return Snapshot();
    }

    private static async Task ExpireSessions(CancellationToken token) {
        while (!token.IsCancellationRequested) {
            await Task.Delay(1000, token).ConfigureAwait(false);
            foreach (var pair in QrLogins) {
                if (pair.Value > DateTimeOffset.UtcNow || (AccountsMap.TryGetValue(pair.Key, out var connectedAccount) && connectedAccount.IsConnectedAndLoggedOn)) continue;
                if (((ICollection<KeyValuePair<string, DateTimeOffset>>)QrLogins).Remove(pair) && AccountsMap.TryGetValue(pair.Key, out var qrAccount) && !qrAccount.IsConnectedAndLoggedOn) {
                    QrCodes.TryRemove(pair.Key, out _);
                    AuthErrors[pair.Key] = "accounts_qr_expired";
                    CancelConnections(pair.Key);
                    qrAccount.Stop(true);
                }
            }
            foreach (var pair in Sessions) {
                var session = pair.Value;
                if (session.Minutes == 0 || session.Games.Length == 0 || DateTimeOffset.UtcNow - session.Started < TimeSpan.FromMinutes(session.Minutes)) continue;
                session.Games = [];
                if (AccountsMap.TryGetValue(pair.Key, out var account)) {
                    try { await EndSession(pair.Key, () => { account.PlayGames([]); return Task.CompletedTask; }, session).ConfigureAwait(false); }
                    catch { account.Stop(true); }
                }
            }
        }
    }

    public static async Task<int> Main(string[] args) {
        using var output = new StreamWriter(Console.OpenStandardOutput(), new UTF8Encoding(false)) { AutoFlush = true };
        Console.SetOut(TextWriter.Null);
        Console.SetError(TextWriter.Null);

        if (args.Length == 1 && args[0] == "--qr-fixture") {
            const string payload = "https://s.team/q/1/1234567890123456789";
            await output.WriteLineAsync(JsonSerializer.Serialize(new { payload, rows = EncodeQr(payload) })).ConfigureAwait(false);
            return 0;
        }

        if (args.Length == 1 && args[0] == "--self-test") {
            if (LoginError(EResult.InvalidPassword) != "accounts_credentials_invalid" || LoginError(EResult.Timeout) != "accounts_connection_failed" || LoginError(EResult.TwoFactorCodeMismatch) != "accounts_guard_invalid" || LoginError(EResult.OK) != null) return 30;
            var settleAt = DateTimeOffset.UtcNow;
            if (LoginSettleDelay(settleAt, settleAt) != TimeSpan.FromSeconds(2) || LoginSettleDelay(settleAt, settleAt.AddSeconds(1)) != TimeSpan.FromSeconds(1) || LoginSettleDelay(settleAt, settleAt.AddSeconds(3)) != TimeSpan.Zero) return 32;

            var conflictAccount = new Account("conflict-fixture", "Fixture", "fixture", [730], RememberLogin: true);
            Accounts[conflictAccount.Id] = conflictAccount;
            Sessions[conflictAccount.Id] = new Session { Games = [730] };
            SessionConflict(conflictAccount.Id);
            if (Sessions[conflictAccount.Id].Games.Length != 0 || !Sessions[conflictAccount.Id].Blocked || !Accounts[conflictAccount.Id].RememberLogin || AuthErrors[conflictAccount.Id] != "accounts_session_conflict") return 31;
            Accounts.Remove(conflictAccount.Id); Sessions.TryRemove(conflictAccount.Id, out _); AuthErrors.TryRemove(conflictAccount.Id, out _);

            var statusPackets = new PresencePacketFixture();
            var isolatedClient = new SteamClient { DebugNetworkListener = statusPackets };
            var friends = isolatedClient.GetHandler<SteamFriends>();
            if (friends == null) return 14;
            foreach (string presence in new[] { "online", "away", "invisible", "offline" }) friends.SetPersonaState(Persona(presence));
            if (statusPackets.Invalid || !statusPackets.States.SequenceEqual(new uint[] { 1, 3, 7, 0 })) return 15;
            if (isolatedClient.IsConnected || friends.GetPersonaState() != EPersonaState.Offline) return 16;

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

            var ids = Enumerable.Range(0, 64).Select(_ => SteamAccount.NewLoginId()).ToArray();
            if (ids.Any(id => id == 0) || ids.Distinct().Count() != ids.Length) return 33;
            string authId = "ll_" + new string('a', 32);
            using (var guardAccount = new SteamAccount(authId, "fixture", EPersonaState.Online)) {
                AccountsMap[authId] = guardAccount;
                guardAccount.Database = new BotDatabase("ll_unused.db") { RefreshToken = "fixture-saved-token" };
                var savedAccount = new Account(authId, "Fixture", "fixture", []);
                SessionConflict(authId);
                if (!CanResume(savedAccount, guardAccount)) return 46;
                AuthenticationFailed(guardAccount, "accounts_credentials_invalid");
                if (CanResume(savedAccount, guardAccount)) return 47;
                AuthErrors.TryRemove(authId, out _);
                guardAccount.KeepRunning = true;
                using var guardCancellation = new CancellationTokenSource();
                var authenticator = new SteamAccount.GuardAuthenticator(guardAccount, guardCancellation.Token);
                var device = authenticator.GetDeviceCodeAsync(false);
                if (!Challenges.TryGetValue(authId, out var challenge) || challenge.Type != EUserInputType.TwoFactorAuthentication) return 34;
                challenge.Completion.TrySetResult("12345");
                if (await device.ConfigureAwait(false) != "12345") return 35;
                var email = authenticator.GetEmailCodeAsync("masked", true);
                if (!Challenges.TryGetValue(authId, out challenge) || challenge.Type != EUserInputType.SteamGuard || AuthErrors[authId] != "accounts_guard_invalid") return 36;
                challenge.Completion.TrySetResult("ABCDE");
                if (await email.ConfigureAwait(false) != "ABCDE") return 37;
                var canceled = authenticator.GetDeviceCodeAsync(false);
                guardCancellation.Cancel();
                try { await canceled.ConfigureAwait(false); return 38; } catch (OperationCanceledException) { }
                if (Challenges.ContainsKey(authId)) return 39;
                Sessions[authId] = new Session { Games = [730] };
                guardAccount.ApplyPlayingState(true, 730);
                if (!guardAccount.IsOccupied || guardAccount.PlayingAppId != 730 || Sessions[authId].Games.Length != 0) return 40;
                guardAccount.ApplyPlayingState(false, 0);
                if (guardAccount.IsOccupied || guardAccount.PlayingAppId != 0) return 41;
                try { guardAccount.PlayGames([730]); return 42; } catch (InvalidOperationException) { }
                AccountsMap.TryRemove(authId, out _);
                Sessions.TryRemove(authId, out _);
                AuthErrors.TryRemove(authId, out _);
            }

            const string secret = "{\"BackingRefreshToken\":\"fixture-refresh-token\",\"BackingSteamGuardData\":\"fixture-guard-secret\"}";
            string encrypted = LootLazyVault.Encode("ll_test.db", secret);
            if (encrypted.Contains("fixture", StringComparison.Ordinal) || LootLazyVault.Decode("ll_test.db", encrypted) != secret) return 2;
            try { LootLazyVault.Decode("ll_test.db", secret); return 3; } catch (CryptographicException) { }

            string fixture = Path.Combine(Path.GetTempPath(), "lootlazy-vault-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(fixture);
            try {
                string database = Path.Combine(fixture, "ll_fixture.db");
                if (!await SerializableFile.Write(database, secret).ConfigureAwait(false)) return 4;
                string persisted = await File.ReadAllTextAsync(database).ConfigureAwait(false);
                if (persisted.Contains("fixture-refresh", StringComparison.Ordinal) || LootLazyVault.Decode(database, persisted) != secret || File.Exists(database + ".new")) return 5;
                var loaded = await BotDatabase.CreateOrLoad(database).ConfigureAwait(false);
                if (loaded == null) return 6;
                loaded.RefreshToken = "rotated-fixture-token";
                await loaded.FlushLootLazyState().ConfigureAwait(false);
                persisted = await File.ReadAllTextAsync(database).ConfigureAwait(false);
                if (persisted.Contains("rotated-fixture-token", StringComparison.Ordinal) || !LootLazyVault.Decode(database, persisted).Contains("rotated-fixture-token", StringComparison.Ordinal)) return 12;
                var reloaded = await BotDatabase.CreateOrLoad(database).ConfigureAwait(false);
                if (reloaded?.RefreshToken != "rotated-fixture-token") return 13;
                Directory.CreateDirectory(database + ".new");
                try {
                    await reloaded.StoreLogin("must-not-replace-token", "must-not-replace-guard").ConfigureAwait(false);
                    return 43;
                } catch (IOException) { }
                if (reloaded.RefreshToken != "rotated-fixture-token" || (await File.ReadAllTextAsync(database).ConfigureAwait(false)) != persisted) return 44;
                Directory.Delete(database + ".new");
                await reloaded.StoreLogin("new-fixture-token", "remembered-guard").ConfigureAwait(false);
                reloaded = await BotDatabase.CreateOrLoad(database).ConfigureAwait(false);
                if (reloaded?.RefreshToken != "new-fixture-token" || reloaded.SteamGuardData != "remembered-guard") return 45;
            } finally { Directory.Delete(fixture, true); }

            await output.WriteLineAsync("Guard challenges/cancellation, unique login IDs, playing-session blocking, transactional token persistence and vault regressions passed.").ConfigureAwait(false);
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

            foreach (string orphan in Directory.EnumerateFiles("config")) {
                string name = Path.GetFileName(orphan);
                string? ownedId = new[] { ".json.new", ".db.new", ".json", ".db" }.Where(suffix => name.EndsWith(suffix, StringComparison.Ordinal)).Select(suffix => name[..^suffix.Length]).FirstOrDefault(ValidId);
                if (ownedId == null || Accounts.ContainsKey(ownedId)) continue;
                LootLazyVault.CheckPath(orphan);
                File.Delete(orphan);
            }

            if (!OfflineTest) {
                foreach (var account in Accounts.Values) {
                    string dbPath = Path.Combine("config", account.Id + ".db");
                    BotDatabase? db = null;
                    if (File.Exists(dbPath)) {
                        db = await BotDatabase.CreateOrLoad(dbPath).ConfigureAwait(false);
                        if (db == null) continue;
                    } else {
                        db = new BotDatabase(dbPath);
                    }
                    var client = new SteamAccount(account.Id, account.Login, Persona(account.Presence));
                    client.Database = db;
                    AccountsMap[account.Id] = client;
                }
                foreach (var account in Accounts.Values) {
                    if (!account.RememberLogin || !AccountsMap.TryGetValue(account.Id, out var client) || !CanResume(account, client)) continue;
                    Restoring[account.Id] = 1;
                    try { client.Start(); } catch { AuthErrors[account.Id] = "accounts_signin_required"; }
                }
            }

            using var cancellation = new CancellationTokenSource();
            var expiry = ExpireSessions(cancellation.Token);
            await output.WriteLineAsync(JsonSerializer.Serialize(new { ready = true, protocol = 1 })).ConfigureAwait(false);

            var reader = Console.In;
            var frame = new StringBuilder();
            var buffer = new char[1];
            while (await reader.ReadAsync(buffer.AsMemory()).ConfigureAwait(false) > 0) {
                char character = buffer[0];
                if (character == '\r') continue;
                if (character != '\n') {
                    if (frame.Length >= 16384) return 2;
                    frame.Append(character);
                    continue;
                }
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
                } finally {
                    frame.Clear();
                }
            }

            await cancellation.CancelAsync().ConfigureAwait(false);
            try { await expiry.ConfigureAwait(false); } catch (OperationCanceledException) { }
            foreach (var challenge in Challenges.Values) challenge.Completion.TrySetResult(null);
            foreach (var client in AccountsMap.Values) client.Dispose();
            return 0;
        } catch { return 2; }
    }
}
