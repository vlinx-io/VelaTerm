import Foundation
import Capacitor
import UIKit
import SafariServices
import AVFoundation
import WebKit
import Security
import Crypto
import Citadel
import NIO
import NIOSSH

private struct RemoteFailure: LocalizedError {
    let message: String
    init(_ message: String) { self.message = message }
    var errorDescription: String? { message }
}

private struct AccountFailure: LocalizedError {
    let code: String
    let message: String
    var errorDescription: String? { message }
}

final class Vault {
    private let lock = NSRecursiveLock()
    private let account: String
    init(account: String = "vault") { self.account = account }
    func update<T>(_ change: (inout [String: Any]) throws -> T) throws -> T {
        lock.lock(); defer { lock.unlock() }
        var store = try read()
        let result = try change(&store)
        try write(store)
        return result
    }
    func read() throws -> [String: Any] {
        lock.lock(); defer { lock.unlock() }
        let query: [String: Any] = [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: "com.velaterm.mobile.remote", kSecAttrAccount as String: account, kSecReturnData as String: true]
        var value: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &value)
        if status == errSecItemNotFound { return ["connections": [[String: Any]](), "keys": [String: String]()] }
        guard status == errSecSuccess, let data = value as? Data, let object = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { throw RemoteFailure(MobileText.get("mobile.native.keychainReadFailed", ["code": String(status)])) }
        return object
    }
    func write(_ object: [String: Any]) throws {
        lock.lock(); defer { lock.unlock() }
        let query: [String: Any] = [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: "com.velaterm.mobile.remote", kSecAttrAccount as String: account]
        let value: [String: Any] = [kSecValueData as String: try JSONSerialization.data(withJSONObject: object), kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly]
        var status = SecItemUpdate(query as CFDictionary, value as CFDictionary)
        if status == errSecItemNotFound { status = SecItemAdd(query.merging(value) { _, new in new } as CFDictionary, nil) }
        guard status == errSecSuccess else { throw RemoteFailure(MobileText.get("mobile.native.keychainWriteFailed", ["code": String(status)])) }
    }
}

private final class HostValidator: NIOSSHClientServerAuthenticationDelegate {
    let verify: (String) async throws -> Void
    init(_ verify: @escaping (String) async throws -> Void) { self.verify = verify }
    func validateHostKey(hostKey: NIOSSHPublicKey, validationCompletePromise: EventLoopPromise<Void>) {
        let representation = String(openSSHPublicKey: hostKey)
        guard let encoded = representation.split(separator: " ").dropFirst().first, let data = Data(base64Encoded: String(encoded)) else { validationCompletePromise.fail(RemoteFailure(MobileText.get("mobile.native.hostKeyUnreadable"))); return }
        let fingerprint = "SHA256:" + Data(SHA256.hash(data: data)).base64EncodedString().replacingOccurrences(of: "=", with: "")
        Task { do { try await verify(fingerprint); validationCompletePromise.succeed(()) } catch { validationCompletePromise.fail(error) } }
    }
}

/// Each accepted TCP connection gets one SSH direct-tcpip channel. Reads are
/// paced by the peer's write completion so terminal output cannot grow without bound.
private final class Relay: ChannelInboundHandler {
    typealias InboundIn = ByteBuffer
    let peer: Channel
    init(peer: Channel) { self.peer = peer }
    func channelActive(context: ChannelHandlerContext) { context.read(); context.fireChannelActive() }
    func handlerAdded(context: ChannelHandlerContext) { if context.channel.isActive { context.read() } }
    func channelRead(context: ChannelHandlerContext, data: NIOAny) {
        let current = context.channel
        peer.writeAndFlush(unwrapInboundIn(data)).whenComplete { result in
            switch result { case .success: current.read(); case .failure: current.close(promise: nil) }
        }
    }
    func channelInactive(context: ChannelHandlerContext) { peer.close(promise: nil) }
    func errorCaught(context: ChannelHandlerContext, error: Error) { context.close(promise: nil); peer.close(promise: nil) }
}

@objc(VelaRemotePlugin)
public class VelaRemotePlugin: CAPPlugin, CAPBridgedPlugin {
    public let identifier = "VelaRemotePlugin"
    public let jsName = "VelaRemote"
    public let pluginMethods: [CAPPluginMethod] = ["list", "save", "remove", "connect", "disconnect", "status", "scanURL", "account", "notifications"].map { CAPPluginMethod(name: $0, returnType: CAPPluginReturnPromise) }
    let vault = Vault()
    private var client: SSHClient?
    private var listener: Channel?
    private var children: [Channel] = []
    private let childrenLock = NSLock()
    private var task: Task<Void, Never>?
    private var generation = 0
    private var activeID: String?
    private var browser: ProjectBrowser?
    private var prompts: TrustPromptCoordinator!
    public override func load() {
        prompts = TrustPromptCoordinator { [weak self] request in guard let self else { return false }; return try await self.presentTrustAlert(request) }
        TaskNotifications.shared.activate()
        TaskNotifications.shared.onOpen = { [weak self] event in self?.notifyListeners("notificationOpen", data: event, retainUntilConsumed: true) }
    }
    @objc func notifications(_ call: CAPPluginCall) {
        Task {
            do {
                switch call.getString("action") ?? "status" {
                case "enable": try await PushRegistration.shared.enable()
                case "disable": try await PushRegistration.shared.disable()
                case "test": try await PushRegistration.shared.test(call.getString("id") ?? "")
                case "status": break
                default: throw PushFailure("PUSH_DISABLED")
                }
                call.resolve(await PushRegistration.shared.status())
            } catch { let code = (error as? PushFailure)?.code ?? "PUSH_RELAY_UNAVAILABLE"; call.reject(code, code) }
        }
    }
    private func string(_ row: [String: Any], _ key: String) -> String { row[key] as? String ?? "" }
    private func number(_ row: [String: Any], _ key: String) -> Int { row[key] as? Int ?? 0 }
    private func port(_ row: [String: Any], _ key: String) throws -> Int {
        let value = number(row, key); guard (1...65535).contains(value) else { throw RemoteFailure(MobileText.get("mobile.native.portRange")) }; return value
    }
    private func safeURL(_ address: String) throws -> URL {
        guard let url = URL(string: address), ["https", "http"].contains(url.scheme ?? ""), let host = url.host, !host.isEmpty, url.user == nil, url.password == nil else { throw RemoteFailure(MobileText.get("mobile.native.addressInvalid")) }
        guard url.scheme != "http" || ["localhost", "127.0.0.1", "[::1]", "::1"].contains(host) else { throw RemoteFailure(MobileText.get("mobile.native.httpsRequired")) }
        return url
    }
    private func validate(_ row: [String: Any]) throws {
        guard !string(row, "name").trimmingCharacters(in: .whitespaces).isEmpty else { throw RemoteFailure(MobileText.get("mobile.native.nameRequired")) }
        if string(row, "mode") == "url" { _ = try safeURL(string(row, "url")); return }
        guard string(row, "mode") == "ssh", !string(row, "host").isEmpty, !string(row, "username").isEmpty, string(row, "host").rangeOfCharacter(from: CharacterSet.whitespacesAndNewlines.union(CharacterSet(charactersIn: "/@"))) == nil else { throw RemoteFailure(MobileText.get("mobile.native.sshHostInvalid")) }
        _ = try port(row, "port")
        guard ["password", "key"].contains(string(row, "auth")), !string(row, string(row, "auth") == "key" ? "privateKey" : "password").isEmpty else { throw RemoteFailure(MobileText.get("mobile.native.sshCredentialsRequired")) }
        if string(row, "service") == "manual" { _ = try port(row, "remotePort") }
        else if string(row, "service") != "auto" { throw RemoteFailure(MobileText.get("mobile.native.serviceModeRequired")) }
    }
    private func record(_ id: String) throws -> [String: Any] {
        guard let row = (try vault.read()["connections"] as? [[String: Any]])?.first(where: { string($0, "id") == id }) else { throw RemoteFailure(MobileText.get("mobile.native.connectionMissing")) }; return row
    }
    private let diagnosticLock = NSLock()
    private var diagnosticOperation = UUID().uuidString
    private var diagnosticPhaseAt = ProcessInfo.processInfo.systemUptime
    private var diagnosticPreviousPhase = "idle"
    private func state(_ phase: String) {
        diagnosticLock.lock()
        let now = ProcessInfo.processInfo.systemUptime
        if phase == "connecting" { diagnosticOperation = UUID().uuidString; diagnosticPhaseAt = now }
        let safePhase = ["connecting", "confirming", "preparing", "forwarding", "ready", "error", "disconnected"].contains(phase) ? phase : "unknown"
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyy-MM-dd HH:mm:ss"
        let level = safePhase == "error" ? "WARN " : "INFO "
        // Never include connection records, URLs, credentials or exception messages.
        let line = "\(formatter.string(from: Date())) [\(level)] [system] event=mobile_connection operationId=\(diagnosticOperation) step=\(safePhase) previousStep=\(diagnosticPreviousPhase) durationMs=\(Int((now-diagnosticPhaseAt)*1000))"
        diagnosticPhaseAt = now
        diagnosticPreviousPhase = safePhase
        diagnosticLock.unlock()
        NSLog("%@", line)
        notifyListeners("state", data: ["phase": phase])
    }
    private var scanningURL = false
    @objc func scanURL(_ call: CAPPluginCall) {
        DispatchQueue.main.async {
            guard !self.scanningURL else { call.reject(MobileText.get("mobile.native.scanBusy"), "SCAN_BUSY"); return }
            self.scanningURL = true
            let presentScanner: (Bool) -> Void = { allowed in
                DispatchQueue.main.async {
                    guard allowed else { self.scanningURL = false; call.reject(MobileText.get("mobile.native.cameraPermissionDenied"), "CAMERA_PERMISSION_DENIED"); return }
                    guard let presenter = self.bridge?.viewController, presenter.presentedViewController == nil else {
                        self.scanningURL = false; call.reject(MobileText.get("mobile.native.scanUnavailable"), "SCAN_UNAVAILABLE"); return
                    }
                    let scanner = URLScanner()
                    scanner.modalPresentationStyle = .fullScreen
                    scanner.completion = { value, error in
                        self.scanningURL = false
                        if let error { call.reject(error, "SCAN_UNAVAILABLE"); return }
                        guard let value else { call.resolve(["cancelled": true]); return }
                        do {
                            guard value.utf8.count <= 8192 else { throw RemoteFailure(MobileText.get("mobile.native.qrTooLong")) }
                            let url = try self.safeURL(value.trimmingCharacters(in: .whitespacesAndNewlines))
                            call.resolve(["url": url.absoluteString, "name": url.host ?? MobileText.get("mobile.native.urlConnectionName")])
                        } catch { call.reject(error.localizedDescription, "INVALID_QR_URL") }
                    }
                    presenter.present(scanner, animated: true)
                }
            }
            switch AVCaptureDevice.authorizationStatus(for: .video) {
            case .authorized: presentScanner(true)
            case .notDetermined: AVCaptureDevice.requestAccess(for: .video, completionHandler: presentScanner)
            default: presentScanner(false)
            }
        }
    }
    private var accountAttempt: [String: String]?
    private weak var accountBrowser: SFSafariViewController?
    private func accountRequest(_ path: String, token: String? = nil, body: [String: Any]? = nil) async throws -> Any {
        var request = URLRequest(url: URL(string: "https://velaterm.com" + path)!)
        request.timeoutInterval = 40
        if let token { request.setValue("Bearer " + token, forHTTPHeaderField: "Authorization") }
        if let body {
            request.httpMethod = "POST"; request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.httpBody = try JSONSerialization.data(withJSONObject: body)
        }
        let (data, response) = try await URLSession.shared.data(for: request)
        guard let response = response as? HTTPURLResponse else { throw RemoteFailure(MobileText.get("mobile.native.accountServiceUnavailable")) }
        if path.hasSuffix("/poll") && [401, 404].contains(response.statusCode) {
            throw AccountFailure(code: "ACCOUNT_LOGIN_EXPIRED", message: MobileText.get("mobile.native.loginRequestExpired"))
        }
        if response.statusCode == 401 { throw AccountFailure(code: "ACCOUNT_AUTH_REQUIRED", message: MobileText.get("mobile.native.sessionExpired")) }
        guard (200..<300).contains(response.statusCode) else { throw RemoteFailure(MobileText.get("mobile.native.accountServiceUnavailable")) }
        if data.isEmpty { return NSNull() }
        return try JSONSerialization.jsonObject(with: data, options: .fragmentsAllowed)
    }
    @MainActor private func accountPage(_ address: String) throws {
        guard let url = URL(string: address), url.scheme == "https", url.host == "velaterm.com", url.user == nil, url.port == nil,
            let presenter = bridge?.viewController, presenter.presentedViewController == nil else { throw RemoteFailure(MobileText.get("mobile.native.accountWindowBusy")) }
        let view = SFSafariViewController(url: url); view.modalPresentationStyle = .fullScreen
        accountBrowser = view; presenter.present(view, animated: true)
    }
    @objc func account(_ call: CAPPluginCall) {
        let action = call.getString("action") ?? "status"
        Task {
            do {
                let saved = try vault.read()
                let token = saved["accountToken"] as? String
                if accountAttempt == nil { accountAttempt = saved["accountAttempt"] as? [String:String] }
                switch action {
                case "login":
                    let key = Curve25519.KeyAgreement.PrivateKey()
                    let name = await MainActor.run { UIDevice.current.name }
                    guard let attempt = try await accountRequest("/api/device-link", body:["name":name,"publicKey":key.publicKey.rawRepresentation.base64EncodedString()]) as? [String:String],
                        let url = attempt["url"], attempt["code"] != nil, attempt["pollToken"] != nil else {throw RemoteFailure(MobileText.get("mobile.native.loginResponseInvalid"))}
                    accountAttempt = attempt
                    try vault.update { $0["accountAttempt"] = attempt }
                    do { try await MainActor.run {try accountPage(url)} }
                    catch { accountAttempt = nil; try vault.update { $0.removeValue(forKey:"accountAttempt") }; throw error }
                    call.resolve(["linked":false])
                case "poll":
                    guard let attempt = accountAttempt, let code = attempt["code"], let poll = attempt["pollToken"], code.range(of:"^[A-Za-z0-9_-]{43}$",options:.regularExpression) != nil else {throw AccountFailure(code:"ACCOUNT_LOGIN_EXPIRED",message:MobileText.get("mobile.native.loginRestart"))}
                    let result = try await accountRequest("/api/device-link/\(code)/poll", token:poll, body:[:])
                    if let value = result as? [String:Any], let credential = value["token"] as? String {
                        try vault.update {$0["accountToken"] = credential; $0.removeValue(forKey:"accountAttempt")}
                        accountAttempt = nil
                        await MainActor.run {accountBrowser?.dismiss(animated:true)}
                        call.resolve(["linked":true])
                    } else {call.resolve(["linked":false])}
                case "status":
                    guard let token else {call.resolve(["linked":false,"pending":accountAttempt != nil]);return}
                    let value = try await accountRequest("/api/device-link/host/status",token:token)
                    var status = value as? [String:Any] ?? ["linked":false]
                    status["pending"] = accountAttempt != nil
                    call.resolve(status)
                case "devices":
                    guard let token else {throw RemoteFailure(MobileText.get("mobile.native.signInFirst"))}
                    call.resolve(["devices":try await accountRequest("/api/device-link/host/remote",token:token)])
                case "logout":
                    guard let token else {call.resolve();return}
                    let push = await PushRegistration.shared.status()
                    for id in push["connections"] as? [String] ?? [] where id.hasPrefix("account_") { try await PushRegistration.shared.revoke(id) }
                    if let status = try await accountRequest("/api/device-link/host/status",token:token) as? [String:Any], status["linked"] as? Bool == true {
                        _ = try await accountRequest("/api/device-link/host/logout",token:token,body:[:])
                    }
                    try vault.update {$0.removeValue(forKey:"accountToken"); $0.removeValue(forKey:"accountAttempt")}
                    accountAttempt = nil; call.resolve()
                case "open":
                    guard let token else {throw RemoteFailure(MobileText.get("mobile.native.signInFirst"))}
                    var body:[String:Any] = [:]
                    if let id = call.getString("deviceId") {guard UUID(uuidString:id) != nil else {throw RemoteFailure(MobileText.get("mobile.native.deviceInvalid"))};body["deviceId"] = id}
                    if let id = call.getString("grantId") {guard UUID(uuidString:id) != nil else {throw RemoteFailure(MobileText.get("mobile.native.grantInvalid"))};body["grantId"] = id}
                    guard let value = try await accountRequest("/api/device-link/host/browser-ticket",token:token,body:body) as? [String:String], let url = value["url"] else {throw RemoteFailure(MobileText.get("mobile.native.connectResponseInvalid"))}
                    if body["deviceId"] != nil || body["grantId"] != nil {
                        try await MainActor.run {
                            guard browser == nil, let address = URL(string: url), address.scheme == "https", address.host == "velaterm.com" else { throw RemoteFailure(MobileText.get("mobile.native.remoteWindowFailed")) }
                            activeID = "account"
                            openBrowser(address, "", "VelaTerm")
                            browser?.requestedSession = call.getString("sessionId")
                        }
                    } else { try await MainActor.run {try accountPage(url)} }
                    call.resolve()
                default: throw RemoteFailure(MobileText.get("mobile.native.accountActionInvalid"))
                }
            } catch let error as AccountFailure {
                if error.code == "ACCOUNT_LOGIN_EXPIRED" {
                    accountAttempt = nil
                    do { try vault.update { $0.removeValue(forKey:"accountAttempt") } }
                    catch { call.reject(error.localizedDescription,"ACCOUNT_ERROR"); return }
                }
                call.reject(error.localizedDescription,error.code)
            } catch {call.reject(error.localizedDescription,"ACCOUNT_ERROR")}
        }
    }
    @objc func list(_ call: CAPPluginCall) {
        do {
            let rows = try vault.update { store in
                let rows = ConnectionRecords.unique(store["connections"] as? [[String: Any]] ?? [])
                store["connections"] = rows
                return rows
            }
            call.resolve(["connections": rows.map(ConnectionRecords.summary)])
        } catch { call.reject(error.localizedDescription, "STORAGE_ERROR") }
    }
    @objc func save(_ call: CAPPluginCall) {
        Task {
            do {
                guard let input = call.getObject("connection") else { throw RemoteFailure(MobileText.get("mobile.native.connectionConfigMissing")) }
                if let id = input["id"] as? String, call.getString("copyFromId") == nil {
                    let rows = try vault.read()["connections"] as? [[String: Any]] ?? []
                    try validate(ConnectionRecords.prepared(input, in: rows))
                    try await PushRegistration.shared.revoke(id)
                }
                let row = try vault.update { store in
                    var rows = store["connections"] as? [[String: Any]] ?? []
                    let copyID = call.getString("copyFromId")
                    let prepared: [String: Any]
                    if let copyID {
                        guard input["id"] == nil, let source = rows.first(where: { string($0, "id") == copyID }), string(source, "mode") == string(input, "mode") else {
                            throw RemoteFailure(MobileText.get("mobile.native.sourceConnectionMissing"))
                        }
                        prepared = ConnectionRecords.copied(input, from: source)
                    } else { prepared = ConnectionRecords.prepared(input, in: rows) }
                    try validate(prepared)
                    let saved = ConnectionRecords.upsert(prepared, into: &rows, preserveEquivalent: copyID != nil)
                    store["connections"] = rows
                    return saved
                }
                call.resolve(["connection": ConnectionRecords.summary(row)])
            } catch { call.reject(error.localizedDescription, "INVALID_CONNECTION") }
        }
    }
    func saveWebPassword(_ password: String, for id: String) throws {
        try vault.update { store in
            var rows = store["connections"] as? [[String: Any]] ?? []
            guard let index = rows.firstIndex(where: { string($0, "id") == id }) else { throw RemoteFailure(MobileText.get("mobile.native.connectionMissing")) }
            var row = rows[index]; row["webPassword"] = password
            _ = ConnectionRecords.upsert(row, into: &rows)
            store["connections"] = rows
        }
    }
    @objc func remove(_ call: CAPPluginCall) {
        Task {
            do {
                try await PushRegistration.shared.revoke(call.getString("id") ?? "")
                try vault.update { store in store["connections"] = (store["connections"] as? [[String: Any]] ?? []).filter { string($0,"id") != call.getString("id") } }; call.resolve()
            } catch { call.reject((error as? PushFailure)?.code ?? error.localizedDescription, "STORAGE_ERROR") }
        }
    }
    // Concurrent challenges for one fingerprint share a single alert; distinct prompts wait until the previous one is dismissed.
    private func approve(_ identity: String, _ fingerprint: String, _ changed: Bool) async -> Bool {
        let epoch = generation
        return await prompts.decide(.init(identity: identity, fingerprint: fingerprint, changed: changed)) { [weak self] in self?.generation == epoch }
    }
    // Top-most controller that is fully in the window hierarchy; retries on the main run loop for about 5 s while a presentation is still animating.
    @MainActor private func settledPresenter() async throws -> UIViewController {
        for _ in 0..<50 {
            var top = bridge?.viewController
            while let next = top?.presentedViewController { top = next }
            if let top, !(top is UIAlertController), !top.isBeingPresented, !top.isBeingDismissed, top.view.window != nil { return top }
            try? await Task.sleep(nanoseconds: 100_000_000)
        }
        throw RemoteFailure("No view controller available for the fingerprint prompt")
    }
    // Shows one fingerprint alert; the continuation is resumed exactly once, also when the alert never appears or vanishes without an action.
    @MainActor private func presentTrustAlert(_ request: TrustPromptCoordinator.Request) async throws -> Bool {
        let host = try await settledPresenter()
        return await withCheckedContinuation { continuation in
            var done = false, shown = false, ticks = 0, gone = 0
            let finish: (Bool) -> Void = { value in guard !done else { return }; done = true; continuation.resume(returning: value) }
            let alert = UIAlertController(title: MobileText.get(request.changed ? "mobile.native.trustChangedTitle" : "mobile.native.trustTitle"), message: MobileText.get(request.changed ? "mobile.native.trustChangedBody" : "mobile.native.trustBody", ["identity": request.identity, "fingerprint": request.fingerprint]), preferredStyle: .alert)
            alert.addAction(UIAlertAction(title: MobileText.get("common.cancel"), style: .cancel) { _ in finish(false) })
            let trust = UIAlertAction(title: MobileText.get("mobile.native.trustAccept"), style: .default) { _ in finish(true) }
            alert.addAction(trust)
            // Trusting a verified fingerprint is the expected action, so it carries the bold highlight instead of Cancel.
            alert.preferredAction = trust
            host.present(alert, animated: true) { shown = true }
            func watch() {
                guard !done else { return }
                ticks += 1; if shown, alert.view.window == nil { gone += 1 }
                // Not on screen after 5 s (presentation refused) or gone for 1 s without an action (dismissed with its presenter): answer false.
                if (!shown && ticks >= 50) || gone >= 10 { finish(false); return }
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.1, execute: watch)
            }
            watch()
        }
    }
    private func resource(_ name: String) throws -> String {
        guard let url = Bundle.module.url(forResource: name, withExtension: nil, subdirectory: "Bootstrap") else { throw RemoteFailure(MobileText.get("mobile.native.resourceMissing")) }; return try String(contentsOf: url, encoding: .utf8)
    }
    private func exec(_ command: String, using client: SSHClient) async throws -> String {
        let output = try await client.executeCommand(command, maxResponseSize: 1024*1024)
        return String(buffer: output).trimmingCharacters(in: .whitespacesAndNewlines)
    }
    private func powershell(_ script: String) -> String { "powershell.exe -NoProfile -NonInteractive -EncodedCommand " + script.data(using: .utf16LittleEndian)!.base64EncodedString() }
    // Detach all old resources before yielding so cleanup cannot close a newer connection.
    private func detachTransport() -> () async -> Void {
        let oldListener = listener; listener = nil; let oldClient = client; client = nil
        let oldChildren = takeChildren()
        return {
            try? await oldListener?.close().get()
            for channel in oldChildren { try? await channel.close().get() }
            try? await oldClient?.close()
        }
    }
    func closeTransport() async { await detachTransport()() }
    private func takeChildren() -> [Channel] { childrenLock.lock(); defer { childrenLock.unlock() }; let result = children; children=[]; return result }
    private func addChild(_ channel: Channel) {
        childrenLock.lock(); children.append(channel); childrenLock.unlock()
        channel.closeFuture.whenComplete { [weak self, weak channel] _ in
            guard let self, let channel else { return }
            self.childrenLock.lock(); self.children.removeAll { $0 === channel }; self.childrenLock.unlock()
        }
    }
    @MainActor func establish(_ row: [String: Any], _ epoch: Int) async throws -> (URL, String) {
        if string(row,"mode") == "url" { return (try safeURL(string(row,"url")), string(row,"webPassword")) }
        state("connecting")
        let host = string(row,"host"), sshPort = try port(row,"port"), username = string(row,"username")
        let identity = "\(host):\(sshPort)"
        let validator = HostValidator { fingerprint in
            let store = try self.vault.read(); let previous = (store["keys"] as? [String: String])?[identity]
            if previous == fingerprint { return }
            self.state("confirming")
            guard self.generation == epoch, await self.approve(identity, fingerprint, previous != nil), self.generation == epoch else { throw RemoteFailure(MobileText.get("mobile.native.hostKeyRejected")) }
            try self.vault.update { updated in var keys = updated["keys"] as? [String: String] ?? [:]; keys[identity] = fingerprint; updated["keys"] = keys }
        }
        let authentication: SSHAuthenticationMethod
        if string(row,"auth") == "key" {
            let text = string(row,"privateKey"), pass = string(row,"passphrase").data(using: .utf8)!
            if let key = try? Curve25519.Signing.PrivateKey(sshEd25519: text, decryptionKey: pass.isEmpty ? nil : pass) { authentication = .ed25519(username: username, privateKey: key) }
            else if (try? Insecure.RSA.PrivateKey(sshRsa: text, decryptionKey: pass.isEmpty ? nil : pass)) != nil { throw RemoteFailure(MobileText.get("mobile.native.rsaUnsupported")) }
            else { throw RemoteFailure(MobileText.get("mobile.native.privateKeyUnreadable")) }
        } else { authentication = .passwordBased(username: username, password: string(row,"password")) }
        let ssh = try await SSHClient.connect(host: host, port: sshPort, authenticationMethod: authentication, hostKeyValidator: .custom(validator), reconnect: .never, connectTimeout: .seconds(120))
        if generation != epoch { try? await ssh.close(); throw RemoteFailure(MobileText.get("mobile.native.connectionCancelled")) }; client = ssh
        func execute(_ command: String) async throws -> String {
            guard self.generation == epoch else { throw RemoteFailure(MobileText.get("mobile.native.connectionCancelled")) }
            return try await self.exec(command, using: ssh)
        }
        state("preparing")
        var servicePort = number(row,"remotePort"), password = string(row,"webPassword")
        if string(row,"service") == "auto" {
            let uname = (try? await execute("uname -s")) ?? ""
            let windows = !["Linux","Darwin","MINGW","CYGWIN"].contains(where: uname.contains)
            let discovery = try JSONSerialization.jsonObject(with: Data(resource("discover.json").utf8)) as! [String: String]
            let result = (try? await execute(windows ? powershell(discovery["windows"]!) : discovery["posix"]!)) ?? ""
            var record = (try? JSONSerialization.jsonObject(with: Data(result.utf8))) as? [String: Any] ?? [:]
            if record["port"] == nil || row["prepare"] as? Bool == true {
                let code = try resource("bootstrap-code.txt").trimmingCharacters(in: .whitespacesAndNewlines)
                let suffix = row["prepare"] as? Bool == true ? " --install" : ""
                let command = "\(windows ? "python" : "python3") -c \"\(code)\"\(suffix)"
                let output = (try? await execute(command)) ?? ""
                guard let object = (try? JSONSerialization.jsonObject(with: Data(output.utf8))) as? [String: Any] else { throw RemoteFailure(MobileText.get("mobile.native.pythonRequired")) }
                record = object
                if let error = object["error"] as? String { throw RemoteFailure(error) }
            }
            servicePort = try port(record,"port"); password = string(record,"password")
        }
        guard generation == epoch else { throw RemoteFailure(MobileText.get("mobile.native.connectionCancelled")) }
        state("forwarding")
        let group = MultiThreadedEventLoopGroup.singleton
        let targetPort = servicePort
        let bootstrap = ServerBootstrap(group: group).serverChannelOption(ChannelOptions.socketOption(.so_reuseaddr), value: 1).childChannelOption(ChannelOptions.autoRead, value: false).childChannelInitializer { local in
            guard self.generation == epoch else { return local.close() }
            self.addChild(local)
            let promise = local.eventLoop.makePromise(of: Void.self)
            Task {
                do {
                    let peer = try await ssh.createDirectTCPIPChannel(using: SSHChannelType.DirectTCPIP(targetHost: "127.0.0.1", targetPort: targetPort, originatorAddress: local.remoteAddress!)) { remote in
                        remote.setOption(ChannelOptions.autoRead, value: false).flatMap { remote.pipeline.addHandler(Relay(peer: local)) }
                    }
                    guard self.generation == epoch else { try? await peer.close().get(); throw RemoteFailure(MobileText.get("mobile.native.connectionCancelled")) }
                    self.addChild(peer)
                    try await local.pipeline.addHandler(Relay(peer: peer)).get()
                    local.read(); peer.read(); promise.succeed(())
                } catch { promise.fail(error); local.close(promise: nil) }
            }
            return promise.futureResult
        }
        var preferred = number(row,"localPort")
        for _ in 0..<100 {
            let candidate = (10000...49151).contains(preferred) ? preferred : Int.random(in: 10000...49151)
            guard generation == epoch else { throw RemoteFailure(MobileText.get("mobile.native.connectionCancelled")) }
            do {
                let bound = try await bootstrap.bind(host:"127.0.0.1",port:candidate).get()
                guard generation == epoch else { try? await bound.close().get(); throw RemoteFailure(MobileText.get("mobile.native.connectionCancelled")) }
                listener = bound; break
            } catch { preferred = 0 }
        }
        guard let localPort = listener?.localAddress?.port else { throw RemoteFailure(MobileText.get("mobile.native.localPortFailed")) }
        try vault.update { store in
            var rows = store["connections"] as? [[String: Any]] ?? []
            if let index = rows.firstIndex(where: { string($0,"id") == string(row,"id") }) { rows[index]["localPort"] = localPort }
            store["connections"] = rows
        }
        let url = URL(string:"http://127.0.0.1:\(localPort)")!
        var request = URLRequest(url: url.appendingPathComponent("api/mode")); request.timeoutInterval = 15
        let (_, response) = try await URLSession.shared.data(for: request)
        guard (response as? HTTPURLResponse)?.statusCode == 200 else { throw RemoteFailure(MobileText.get("mobile.native.healthCheckFailed")) }
        return (url,password)
    }
    @objc func connect(_ call: CAPPluginCall) {
        guard let id = call.getString("id") else { call.reject(MobileText.get("mobile.native.connectionIdMissing")); return }
        generation += 1; let epoch = generation; task?.cancel(); activeID = id
        let cleanup = detachTransport()
        task = Task { @MainActor in
            await cleanup()
            do {
                guard generation == epoch else { throw RemoteFailure(MobileText.get("mobile.native.connectionCancelled")) }
                let row = try record(id); var (url,password) = try await establish(row,epoch)
                if let session = call.getString("sessionId"), !session.isEmpty, session.utf8.count <= 256 {
                    var parts = URLComponents(url: url, resolvingAgainstBaseURL: false)!
                    var query = parts.queryItems ?? []; query.removeAll { $0.name == "session" }; query.append(URLQueryItem(name: "session", value: session)); parts.queryItems = query
                    url = parts.url ?? url
                }
                guard generation == epoch else { throw RemoteFailure(MobileText.get("mobile.native.connectionCancelled")) }
                self.openBrowser(url,password,string(row,"name")); state("ready"); call.resolve(["id":id])
            } catch { if generation == epoch { let cleanup = detachTransport(); state("error"); Task { await cleanup() } }; call.reject(error.localizedDescription,"REMOTE_ERROR") }
        }
    }
    @objc func disconnect(_ call: CAPPluginCall) {
        generation += 1; task?.cancel(); activeID = nil
        let cleanup = detachTransport()
        let current = browser; browser = nil
        current?.stopLoading(); current?.dismiss(animated: false)
        state("disconnected"); call.resolve()
        Task { await cleanup() }
    }
    @objc func status(_ call: CAPPluginCall) { call.resolve(["connected":client?.isConnected ?? (browser != nil),"id":activeID ?? ""]) }
    @MainActor private func openBrowser(_ url: URL, _ password: String, _ title: String) {
        if let existing = browser { existing.load(url,password:password); return }
        let view = ProjectBrowser(url:url,password:password,title:title)
        view.connectionID = activeID
        let connectionID = activeID
        view.onSavePassword = { [weak self] value in
            guard let self, let id = connectionID, self.activeID == id, self.browser != nil else { throw RemoteFailure(MobileText.get("mobile.native.connectionClosed")) }
            try self.saveWebPassword(value, for: id)
        }
        view.onClose = { [weak self, weak view] in
            guard let self, self.browser === view else { return }
            self.generation += 1; self.task?.cancel(); self.browser = nil; self.activeID = nil
            let cleanup = self.detachTransport()
            self.state("disconnected")
            Task { await cleanup() }
        }
        view.onReconnect = { [weak self, weak view] in
            guard let self, let id = self.activeID, self.browser === view else { return }
            if id == "account" { view?.reloadAccount(); return }
            self.generation += 1; let epoch = self.generation; self.task?.cancel()
            let cleanup = self.detachTransport()
            self.task = Task { @MainActor in
                await cleanup()
                do {
                    guard self.generation == epoch else { return }
                    let row = try self.record(id); let (next, secret) = try await self.establish(row, epoch)
                    guard self.generation == epoch, self.browser === view else { return }
                    view?.load(next, password: secret)
                } catch {
                    guard self.generation == epoch, self.browser === view else { return }
                    let cleanup = self.detachTransport()
                    view?.showError(error.localizedDescription)
                    await cleanup()
                }
            }
        }
        view.onCertificate = { [weak self, weak view] identity, fingerprint in
            guard let self else { return false }
            // A declined or failed prompt explains itself on the page, like Android does, instead of waiting for WebKit's error, which some builds report as a plain cancellation.
            let declined: () async -> Bool = { await MainActor.run { view?.showError(MobileText.get("mobile.native.certificateRejected")) }; return false }
            do {
                let store=try self.vault.read();let previous=(store["keys"] as? [String:String])?[identity]
                if previous==fingerprint {return true}
                guard await self.approve(MobileText.get("mobile.native.tlsIdentity", ["identity": identity]),fingerprint,previous != nil) else{return await declined()}
                try self.vault.update { updated in var keys=updated["keys"] as? [String:String] ?? [:];keys[identity]=fingerprint;updated["keys"]=keys };return true
            } catch {return await declined()}
        }
        browser=view;view.modalPresentationStyle = .fullScreen
        bridge?.viewController?.present(view,animated:true)
    }
}

private final class ProjectBrowser: UIViewController, WKNavigationDelegate, WKUIDelegate, WKDownloadDelegate, UIAdaptivePresentationControllerDelegate {
    var connectionID: String?
    var requestedSession: String?
    var onSavePassword: ((String) throws -> Void)?
    var onClose: (() -> Void)?
    var onReconnect: (() -> Void)?
    var onCertificate: ((String,String) async -> Bool)?
    private var target: URL
    private var password: String
    private var web: WKWebView!
    private let message = UILabel()
    private let recovery = ConnectionRecoveryView()
    private var navigationFailed = false
    // Fingerprints the user declined during the current load: WebKit opens further connections after a refusal, and each would ask again. Cleared when a new load starts (retry).
    private var declinedFingerprints = Set<String>()
    private var reconnecting = false
    private var loadTimeout: DispatchWorkItem?
    private var closed = false
    private var downloads: [ObjectIdentifier: URL] = [:]
    init(url:URL,password:String,title:String) {target=url;self.password=password;super.init(nibName:nil,bundle:nil);self.title=title}
    required init?(coder:NSCoder) {fatalError("Use init(url:password:title:)")}
    func reloadAccount() { beginLoading(); web?.reload() }
    private var notificationConnection: String? {
        guard connectionID == "account" else { return connectionID }
        guard let url = web?.url, url.host == "velaterm.com", url.scheme == "https" else { return nil }
        let parts = url.pathComponents
        guard parts.count >= 3, parts[1] == "r", UUID(uuidString: parts[2]) != nil else { return nil }
        return "account_" + parts[2].lowercased()
    }
    override func viewDidLoad() {
        super.viewDidLoad(); view.backgroundColor = .systemBackground
        let configuration=WKWebViewConfiguration();configuration.websiteDataStore = .nonPersistent()
        configuration.userContentController.addScriptMessageHandler(PageReadyMessageHandler(self), contentWorld: .page, name: "VelaPageReady")
        configuration.userContentController.addScriptMessageHandler(PasswordMessageHandler(self), contentWorld: .page, name: "VelaPassword")
        configuration.userContentController.addScriptMessageHandler(NotificationMessageHandler(self), contentWorld: .page, name: "VelaNotifications")
        web=WKWebView(frame:.zero,configuration:configuration);web.navigationDelegate=self;web.uiDelegate=self;web.customUserAgent=nil
        web.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(web)
        NSLayoutConstraint.activate([web.topAnchor.constraint(equalTo:view.safeAreaLayoutGuide.topAnchor),web.leadingAnchor.constraint(equalTo:view.leadingAnchor),web.trailingAnchor.constraint(equalTo:view.trailingAnchor)])
        let webBottom = web.bottomAnchor.constraint(equalTo:view.keyboardLayoutGuide.topAnchor)
        webBottom.isActive = true
        if connectionID == "account" {
            let back = UIButton(type: .system)
            back.setTitle(MobileText.get("mobile.backConnections"), for: .normal)
            back.titleLabel?.font = .systemFont(ofSize: 16, weight: .medium)
            back.backgroundColor = .secondarySystemBackground
            back.translatesAutoresizingMaskIntoConstraints = false; view.addSubview(back)
            back.addAction(UIAction { [weak self] _ in self?.close() }, for: .touchUpInside)
            NSLayoutConstraint.activate([back.bottomAnchor.constraint(equalTo: view.keyboardLayoutGuide.topAnchor), back.leadingAnchor.constraint(equalTo: view.leadingAnchor), back.trailingAnchor.constraint(equalTo: view.trailingAnchor), back.heightAnchor.constraint(equalToConstant: 48)])
            webBottom.constant = -48
        }
        recovery.translatesAutoresizingMaskIntoConstraints = false; view.addSubview(recovery)
        NSLayoutConstraint.activate([recovery.topAnchor.constraint(equalTo: web.topAnchor), recovery.bottomAnchor.constraint(equalTo: web.bottomAnchor), recovery.leadingAnchor.constraint(equalTo: web.leadingAnchor), recovery.trailingAnchor.constraint(equalTo: web.trailingAnchor)])
        recovery.onBack = { [weak self] in self?.close() }
        recovery.onRetry = { [weak self] in self?.reconnect() }
        load(target,password:password)
        NotificationCenter.default.addObserver(self,selector:#selector(resumed),name:UIApplication.willEnterForegroundNotification,object:nil)
    }
    @objc private func showConnectionPanel() {
        let details = [title ?? MobileText.get("mobile.native.currentServer"), message.text == title ? nil : message.text].compactMap { $0 }.joined(separator:"\n")
        let panel = UIAlertController(title:MobileText.get("mobile.connections"), message:details, preferredStyle:.actionSheet)
        panel.addAction(UIAlertAction(title:MobileText.get("mobile.native.reconnect"), style:.default) { [weak self] _ in self?.reconnect() })
        panel.addAction(UIAlertAction(title:MobileText.get("mobile.native.switchConnection"), style:.default) { [weak self] _ in self?.close() })
        panel.addAction(UIAlertAction(title:MobileText.get("common.cancel"), style:.cancel))
        panel.popoverPresentationController?.sourceView = view
        panel.popoverPresentationController?.sourceRect = CGRect(x:view.bounds.maxX-44,y:view.safeAreaInsets.top,width:44,height:44)
        present(panel, animated:true)
    }
    func load(_ url:URL,password:String) {
        guard !closed else { return }
        reconnecting=false; navigationFailed=false
        if let current=web?.url,current.host==target.host {
            var restored=URLComponents(url:url,resolvingAgainstBaseURL:false)!
            let previous=URLComponents(url:current,resolvingAgainstBaseURL:false)!
            restored.path=previous.path;restored.query=previous.query;restored.fragment=previous.fragment
            target=restored.url ?? url
        } else {target=url}
        self.password=password;guard web != nil else{return}
        web.configuration.userContentController.removeAllUserScripts()
        let secret=String(data:try! JSONSerialization.data(withJSONObject:["password":password]),encoding:.utf8)!
        let origin="\(url.scheme!)://\(url.host!)\(url.port.flatMap { $0 == (url.scheme == "https" ? 443 : 80) ? nil : ":\($0)" } ?? "")"
        let expected=String(data:try! JSONSerialization.data(withJSONObject:origin,options:.fragmentsAllowed),encoding:.utf8)!
        let loginURL = Bundle.module.url(forResource: "login", withExtension: "js", subdirectory: "Bootstrap")!
        let loginScript = (try? String(contentsOf: loginURL, encoding: .utf8)) ?? ""
        let notificationURL = Bundle.module.url(forResource: "notifications", withExtension: "js", subdirectory: "Bootstrap")!
        let notificationScript = (try? String(contentsOf: notificationURL, encoding: .utf8)) ?? ""
        let readinessURL = Bundle.module.url(forResource: "page-readiness", withExtension: "js", subdirectory: "Bootstrap")!
        let readinessScript = (try? String(contentsOf: readinessURL, encoding: .utf8)) ?? ""
        let script="if(location.origin===\(expected)){window.__VELATERM_CONNECTION_MENU__=true;window.__VLX_AUTOLOGIN__=\(secret);\(loginScript)\n\(notificationScript)\n\(readinessScript)}"
        web.configuration.userContentController.addUserScript(WKUserScript(source:script,injectionTime:.atDocumentStart,forMainFrameOnly:true))
        beginLoading(); message.text=title;web.load(URLRequest(url:target))
    }
    private func beginLoading() {
        guard !closed else { return }
        loadTimeout?.cancel(); navigationFailed = false; declinedFingerprints.removeAll(); recovery.showLoading()
        let timeout = DispatchWorkItem { [weak self] in
            guard let self, !self.closed, !self.navigationFailed else { return }
            self.reconnecting = false
            self.recovery.showLoading(MobileText.get("mobile.loadSlow"), allowRetry: true)
        }
        loadTimeout = timeout
        DispatchQueue.main.asyncAfter(deadline: .now() + 30, execute: timeout)
    }
    func pageReady(_ message: WKScriptMessage, reply: @escaping (Any?, String?) -> Void) {
        let origin = message.frameInfo.securityOrigin
        guard !closed, !navigationFailed, message.frameInfo.isMainFrame,
              origin.protocol == target.scheme, origin.host == target.host,
              (origin.port == 0 ? (origin.protocol == "https" ? 443 : 80) : origin.port) == (target.port ?? (target.scheme == "https" ? 443 : 80)),
              (message.body as? [String: Any])?["ready"] as? Bool == true else { reply(nil, "Page unavailable"); return }
        loadTimeout?.cancel(); loadTimeout = nil; recovery.isHidden = true; reconnecting = false
        reply(true, nil)
    }
    func stopLoading() {
        closed = true; loadTimeout?.cancel(); loadTimeout = nil; web?.stopLoading()
    }
    func notification(_ message: WKScriptMessage, reply: @escaping (Any?, String?) -> Void) {
        let origin = message.frameInfo.securityOrigin
        guard message.frameInfo.isMainFrame, origin.protocol == target.scheme, origin.host == target.host,
              (origin.port == 0 ? (origin.protocol == "https" ? 443 : 80) : origin.port) == (target.port ?? (target.scheme == "https" ? 443 : 80)),
              !closed, let body = message.body as? [String: Any], let connectionID = notificationConnection else { reply(nil, "Notifications unavailable"); return }
        switch body["action"] as? String {
        case "permission": TaskNotifications.shared.permission(request: body["request"] as? Bool == true) { value in DispatchQueue.main.async { reply(value, nil) } }
        case "send": Task {
            if await PushRegistration.shared.isBound(connectionID) { await MainActor.run { reply(true, nil) }; return }
            TaskNotifications.shared.send(connectionID: connectionID, body: body) { error in DispatchQueue.main.async { reply(error == nil ? true : nil, error) } }
        }
        case "subscription": Task {
            do { let value = try await PushRegistration.shared.subscription(connectionID); await MainActor.run {
                guard !self.closed, self.notificationConnection == connectionID else { reply(nil, "Notifications unavailable"); return }
                reply(value, nil)
            } }
            catch { await MainActor.run { reply(nil, (error as? PushFailure)?.code ?? "PUSH_RELAY_UNAVAILABLE") } }
        }
        case "bound": Task {
            do { try await PushRegistration.shared.bound(connectionID, active: body["active"] as? Bool == true); await MainActor.run { reply(true, nil) } }
            catch { await MainActor.run { reply(nil, "PUSH_STORAGE_FAILED") } }
        }
        default: reply(nil, "Unknown notification action")
        }
    }
    func savePassword(_ message: WKScriptMessage, reply: @escaping (Any?, String?) -> Void) {
        let origin = message.frameInfo.securityOrigin
        guard message.frameInfo.isMainFrame, origin.protocol == target.scheme, origin.host == target.host,
              (origin.port == 0 ? (origin.protocol == "https" ? 443 : 80) : origin.port) == (target.port ?? (target.scheme == "https" ? 443 : 80)),
              let body = message.body as? [String: Any], let value = body["password"] as? String, value.utf8.count <= 4096,
              let save = onSavePassword else { reply(nil, "Password storage unavailable"); return }
        do { try save(value); password = value; reply(true, nil) }
        catch { reply(nil, "Password storage failed") }
    }
    func showError(_ text:String) {
        guard !closed else { return }
        loadTimeout?.cancel(); loadTimeout = nil
        reconnecting=false; navigationFailed=true; message.text=text
        recovery.show(text); view.endEditing(true)
    }
    private func showDownloadError(_ text: String) {
        message.text = text
        let alert = UIAlertController(title: MobileText.get("mobile.native.downloadFailedTitle"), message: text, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: MobileText.get("mobile.native.ok"), style: .cancel))
        if presentedViewController == nil { present(alert, animated: true) }
    }
    @objc private func resumed() { reconnect() }
    @objc private func reconnect() {
        guard !closed, !reconnecting else{return}; reconnecting=true
        web.stopLoading(); beginLoading()
        onReconnect?()
    }
    @objc private func close() {stopLoading(); dismiss(animated:false);onClose?()}
    func webView(_ webView:WKWebView,decidePolicyFor action:WKNavigationAction,decisionHandler:@escaping(WKNavigationActionPolicy)->Void) {
        guard let url=action.request.url else {decisionHandler(.cancel);return}
        if url.scheme == "velaterm-ui", url.host == "close", action.targetFrame?.isMainFrame == true {
            decisionHandler(.cancel); close(); return
        }
        if url.scheme == "velaterm-ui", url.host == "connections", action.targetFrame?.isMainFrame == true {
            decisionHandler(.cancel)
            if presentedViewController == nil { showConnectionPanel() }
            return
        }
        let sameOrigin=url.scheme==target.scheme && url.host==target.host && (url.port ?? (url.scheme=="https" ? 443:80)) == (target.port ?? (target.scheme=="https" ? 443:80))
        let localDownload=action.shouldPerformDownload && ["blob","data"].contains(url.scheme ?? "")
        if sameOrigin, action.targetFrame?.isMainFrame == true, connectionID == "account", url.path.hasPrefix("/r/"),
           let session = requestedSession {
            requestedSession = nil
            if session.range(of: "^[A-Za-z0-9_-]{1,128}$", options: .regularExpression) != nil {
                var parts = URLComponents(url: url, resolvingAgainstBaseURL: false)!
                var query = parts.queryItems ?? []; query.removeAll { $0.name == "session" }; query.append(URLQueryItem(name: "session", value: session)); parts.queryItems = query
                decisionHandler(.cancel); webView.load(URLRequest(url: parts.url!)); return
            }
        }
        let allowed=sameOrigin || localDownload
        if !allowed && action.targetFrame?.isMainFrame != false {message.text=MobileText.get("mobile.native.navigationBlocked", ["host": url.host ?? url.scheme ?? ""])}
        decisionHandler(allowed ? (action.shouldPerformDownload ? .download:.allow):.cancel)
    }
    func webView(_ webView:WKWebView,decidePolicyFor response:WKNavigationResponse,decisionHandler:@escaping(WKNavigationResponsePolicy)->Void) {
        if response.isForMainFrame, let http = response.response as? HTTPURLResponse, http.statusCode >= 400 {
            showError(MobileText.get("mobile.native.pageUnavailable", ["code": String(http.statusCode)]))
            decisionHandler(.cancel); return
        }
        decisionHandler(response.canShowMIMEType ? .allow:.download)
    }
    func webView(_ webView:WKWebView,navigationAction:WKNavigationAction,didBecome download:WKDownload) {download.delegate=self}
    func webView(_ webView:WKWebView,navigationResponse:WKNavigationResponse,didBecome download:WKDownload) {download.delegate=self}
    func download(_ download:WKDownload,decideDestinationUsing response:URLResponse,suggestedFilename:String,completionHandler:@escaping(URL?)->Void) {
        do {
            let directory=FileManager.default.temporaryDirectory.appendingPathComponent("vela-download-"+UUID().uuidString,isDirectory:true)
            try FileManager.default.createDirectory(at:directory,withIntermediateDirectories:true)
            let name=(suggestedFilename as NSString).lastPathComponent
            let file=directory.appendingPathComponent(name.isEmpty || name=="." || name==".." ? "download":name)
            downloads[ObjectIdentifier(download)]=file;completionHandler(file)
        } catch {showDownloadError(MobileText.get("mobile.native.downloadCreateFailed"));completionHandler(nil)}
    }
    func downloadDidFinish(_ download:WKDownload) {
        guard let file=downloads.removeValue(forKey:ObjectIdentifier(download)) else{return}
        let picker=UIActivityViewController(activityItems:[file],applicationActivities:nil)
        picker.completionWithItemsHandler={_,_,_,_ in try? FileManager.default.removeItem(at:file.deletingLastPathComponent())}
        picker.popoverPresentationController?.sourceView=view;present(picker,animated:true)
    }
    func download(_ download:WKDownload,didFailWithError error:Error,resumeData:Data?) {
        if let file=downloads.removeValue(forKey:ObjectIdentifier(download)) {try? FileManager.default.removeItem(at:file.deletingLastPathComponent())}
        showDownloadError(MobileText.get("mobile.native.downloadFileFailed"))
    }
    func webView(_ webView:WKWebView,runJavaScriptAlertPanelWithMessage text:String,initiatedByFrame frame:WKFrameInfo,completionHandler:@escaping()->Void) {
        let alert=UIAlertController(title:title,message:text,preferredStyle:.alert)
        alert.addAction(UIAlertAction(title:MobileText.get("mobile.native.ok"),style:.default){_ in completionHandler()});present(alert,animated:true)
    }
    func webView(_ webView:WKWebView,runJavaScriptConfirmPanelWithMessage text:String,initiatedByFrame frame:WKFrameInfo,completionHandler:@escaping(Bool)->Void) {
        let alert=UIAlertController(title:title,message:text,preferredStyle:.alert)
        alert.addAction(UIAlertAction(title:MobileText.get("common.cancel"),style:.cancel){_ in completionHandler(false)})
        alert.addAction(UIAlertAction(title:MobileText.get("mobile.native.ok"),style:.default){_ in completionHandler(true)});present(alert,animated:true)
    }
    func webView(_ webView:WKWebView,runJavaScriptTextInputPanelWithPrompt prompt:String,defaultText:String?,initiatedByFrame frame:WKFrameInfo,completionHandler:@escaping(String?)->Void) {
        let alert=UIAlertController(title:title,message:prompt,preferredStyle:.alert);alert.addTextField{$0.text=defaultText}
        alert.addAction(UIAlertAction(title:MobileText.get("common.cancel"),style:.cancel){_ in completionHandler(nil)})
        alert.addAction(UIAlertAction(title:MobileText.get("mobile.native.ok"),style:.default){_ in completionHandler(alert.textFields?.first?.text)});present(alert,animated:true)
    }
    func webView(_ webView:WKWebView,createWebViewWith configuration:WKWebViewConfiguration,for action:WKNavigationAction,windowFeatures:WKWindowFeatures)->WKWebView? {
        if action.targetFrame == nil,let url=action.request.url,url.scheme==target.scheme,url.host==target.host,(url.port ?? (url.scheme=="https" ? 443:80))==(target.port ?? (target.scheme=="https" ? 443:80)) {webView.load(action.request)}
        return nil
    }
    func webView(_ webView:WKWebView,didReceive challenge:URLAuthenticationChallenge,completionHandler:@escaping(URLSession.AuthChallengeDisposition,URLCredential?)->Void) {
        guard challenge.protectionSpace.authenticationMethod==NSURLAuthenticationMethodServerTrust,
              challenge.protectionSpace.host==target.host,
              challenge.protectionSpace.port==(target.port ?? 443),
              let trust=challenge.protectionSpace.serverTrust else {completionHandler(.performDefaultHandling,nil);return}
        if SecTrustEvaluateWithError(trust,nil) {completionHandler(.performDefaultHandling,nil);return}
        guard let chain=SecTrustCopyCertificateChain(trust) as? [SecCertificate],let leaf=chain.first else {completionHandler(.cancelAuthenticationChallenge,nil);return}
        let fingerprint="SHA256:"+Data(SHA256.hash(data:SecCertificateCopyData(leaf) as Data)).base64EncodedString().replacingOccurrences(of:"=",with:"")
        let identity="tls:https://\(target.host!):\(target.port ?? 443)"
        if declinedFingerprints.contains(fingerprint) {completionHandler(.cancelAuthenticationChallenge,nil);return}
        Task {let accepted=await onCertificate?(identity,fingerprint) ?? false;await MainActor.run {if !accepted {self.declinedFingerprints.insert(fingerprint)};completionHandler(accepted ? .useCredential:.cancelAuthenticationChallenge,accepted ? URLCredential(trust:trust):nil)}}
    }
    private func failedNavigation(_ error: Error) {
        let failure = error as NSError
        if failure.domain == NSURLErrorDomain && failure.code == NSURLErrorCancelled { return }
        // The first failure of a load owns the page; a declined fingerprint prompt has already explained itself.
        if navigationFailed { return }
        // Append the system sentence (already localized by iOS) plus domain and code so the real cause is visible on the page.
        showError(MobileText.get("mobile.native.pageLoadFailedReason", ["reason": failure.localizedDescription, "domain": failure.domain, "code": String(failure.code)]))
    }
    func webView(_ webView:WKWebView,didFailProvisionalNavigation navigation:WKNavigation!,withError error:Error) { failedNavigation(error) }
    func webView(_ webView:WKWebView,didFail navigation:WKNavigation!,withError error:Error) { failedNavigation(error) }
    func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) { beginLoading() }
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) { showError(MobileText.get("mobile.native.pageTerminated")) }
    deinit {loadTimeout?.cancel(); NotificationCenter.default.removeObserver(self)}
}

// WKUserContentController retains its handlers; keep the browser reference weak.
private final class PasswordMessageHandler: NSObject, WKScriptMessageHandlerWithReply {
    weak var browser: ProjectBrowser?
    init(_ browser: ProjectBrowser) { self.browser = browser }
    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage, replyHandler: @escaping (Any?, String?) -> Void) {
        guard let browser else { replyHandler(nil, "Connection closed"); return }
        browser.savePassword(message, reply: replyHandler)
    }
}

private final class NotificationMessageHandler: NSObject, WKScriptMessageHandlerWithReply {
    weak var browser: ProjectBrowser?
    init(_ browser: ProjectBrowser) { self.browser = browser }
    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage, replyHandler: @escaping (Any?, String?) -> Void) {
        guard let browser else { replyHandler(nil, "Connection closed"); return }
        browser.notification(message, reply: replyHandler)
    }
}

private final class PageReadyMessageHandler: NSObject, WKScriptMessageHandlerWithReply {
    weak var browser: ProjectBrowser?
    init(_ browser: ProjectBrowser) { self.browser = browser }
    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage, replyHandler: @escaping (Any?, String?) -> Void) {
        guard let browser else { replyHandler(nil, "Connection closed"); return }
        browser.pageReady(message, reply: replyHandler)
    }
}
