import XCTest
import UIKit
import WebKit
import Capacitor
@testable import VelaRemotePlugin

/// Opt-in end-to-end check of the fingerprint prompt against a real VelaTerm desktop with a self-signed
/// certificate, driven inside the running app on a simulator. Set `VELA_E2E_URL` (for example
/// `https://127.0.0.1:8799/`) in the test runner environment; the tests are skipped otherwise.
final class TrustPromptE2ETests: XCTestCase {
    private var target: String { ProcessInfo.processInfo.environment["VELA_E2E_URL"] ?? "" }

    // MARK: plumbing

    @MainActor private func bridgeController() throws -> CAPBridgeViewController {
        try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.flatMap(\.windows)
            .first { $0.isKeyWindow }?.rootViewController as? CAPBridgeViewController)
    }

    @MainActor private func invoke(_ plugin: VelaRemotePlugin, _ method: String, _ options: [String: Any]) async throws -> [String: Any] {
        try await withCheckedThrowingContinuation { continuation in
            let call = CAPPluginCall(callbackId: UUID().uuidString, methodName: method, options: options, success: { result, _ in
                continuation.resume(returning: result?.data ?? [:])
            }, error: { error in
                continuation.resume(throwing: NSError(domain: error?.code ?? method, code: 1, userInfo: [NSLocalizedDescriptionKey: error?.message ?? "\(method) failed"]))
            })!
            switch method {
            case "save": plugin.save(call)
            case "connect": plugin.connect(call)
            case "disconnect": plugin.disconnect(call)
            default: continuation.resume(throwing: NSError(domain: "e2e", code: 2, userInfo: [NSLocalizedDescriptionKey: "unknown method \(method)"]))
            }
        }
    }

    /// Every controller in the presentation chain starting at the bridge controller.
    @MainActor private func chain() -> [UIViewController] {
        var result: [UIViewController] = []
        var current = try? bridgeController() as UIViewController
        while let controller = current { result.append(controller); current = controller.presentedViewController }
        return result
    }

    @MainActor private func alerts() -> [UIAlertController] { chain().compactMap { $0 as? UIAlertController } }

    @MainActor private func find<T: UIView>(_ type: T.Type, in view: UIView) -> T? {
        if let match = view as? T { return match }
        for child in view.subviews { if let match = find(type, in: child) { return match } }
        return nil
    }

    @MainActor private func findNamed(_ name: String, in view: UIView) -> UIView? {
        if String(describing: Swift.type(of: view)) == name { return view }
        for child in view.subviews { if let match = findNamed(name, in: child) { return match } }
        return nil
    }

    @MainActor private func browserView() -> UIView? {
        chain().first { String(describing: Swift.type(of: $0)) == "ProjectBrowser" }?.view
    }

    @MainActor private func labels(in view: UIView) -> [String] {
        var result: [String] = []
        if let label = view as? UILabel, let text = label.text, !text.isEmpty { result.append(text) }
        for child in view.subviews { result.append(contentsOf: labels(in: child)) }
        return result
    }

    @MainActor private func waitFor<T>(_ what: String, seconds: Double, _ probe: @MainActor () -> T?) async throws -> T {
        let deadline = Date().addingTimeInterval(seconds)
        while Date() < deadline {
            if let value = probe() { return value }
            try await Task.sleep(for: .milliseconds(100))
        }
        XCTFail("timed out after \(seconds)s waiting for \(what)")
        throw NSError(domain: "e2e", code: 3, userInfo: [NSLocalizedDescriptionKey: "timed out waiting for \(what)"])
    }

    /// Runs an alert action the way UIKit does after a tap: dismiss, then the handler. The handler is
    /// private API, acceptable in a test that cannot tap.
    @MainActor private func tap(_ action: UIAlertAction, on alert: UIAlertController) throws {
        typealias Handler = @convention(block) (UIAlertAction) -> Void
        let raw = try XCTUnwrap(action.value(forKey: "handler"), "action has no handler")
        alert.dismiss(animated: false)
        unsafeBitCast(raw as AnyObject, to: Handler.self)(action)
    }

    @MainActor private func start(_ plugin: VelaRemotePlugin) async throws -> String {
        // Start from a clean screen: no browser or alert left over from a previous test.
        _ = try? await invoke(plugin, "disconnect", [:])
        _ = try await waitFor("previous presentations to go away", seconds: 5) { self.chain().count == 1 ? true : nil }
        // Forget stored fingerprints so the prompt must appear.
        try plugin.vault.update { store in store["keys"] = [String: String]() }
        var connection = JSObject(); connection["name"] = "E2E trust prompt"; connection["mode"] = "url"; connection["url"] = target
        var options = JSObject(); options["connection"] = connection
        let saved = try await invoke(plugin, "save", options)
        let id = try XCTUnwrap((saved["connection"] as? [String: Any])?["id"] as? String)
        _ = try await invoke(plugin, "connect", ["id": id])
        return id
    }

    @MainActor private func describe(_ alert: UIAlertController) {
        NSLog("E2E alert title=%@ | message=%@ | actions=%@ | preferred=%@", alert.title ?? "nil", alert.message ?? "nil",
              alert.actions.map { "\($0.title ?? "nil")[\($0.style.rawValue)]" }.joined(separator: " / "), alert.preferredAction?.title ?? "nil")
    }

    // MARK: tests

    @MainActor func testPromptsOnceAndLoadsAfterTrust() async throws {
        guard !target.isEmpty else { throw XCTSkip("Set VELA_E2E_URL to a VelaTerm desktop with a self-signed certificate") }
        let plugin = try XCTUnwrap(bridgeController().bridge?.plugin(withName: "VelaRemote") as? VelaRemotePlugin)
        let original = try plugin.vault.read()
        defer { try? plugin.vault.write(original) }
        _ = try await start(plugin)

        let alert = try await waitFor("the fingerprint alert", seconds: 15) { self.alerts().first }
        describe(alert)
        XCTAssertEqual(alert.actions.count, 2)
        let confirm = try XCTUnwrap(alert.actions.first { $0.style == .default }, "confirm action must be .default, not .destructive")
        let cancel = try XCTUnwrap(alert.actions.first { $0.style == .cancel })
        XCTAssertEqual(alert.preferredAction, confirm, "trusting a verified fingerprint is the highlighted action")
        XCTAssertNotEqual(confirm.title, cancel.title)
        XCTAssertEqual(alert.title, MobileText.get("mobile.native.trustTitle"))
        XCTAssertEqual(confirm.title, MobileText.get("mobile.native.trustAccept"))
        XCTAssertFalse((alert.message ?? "").isEmpty)
        XCTAssertTrue((alert.message ?? "").contains("SHA256:"), "the fingerprint is shown for comparison")

        // WebKit raises further challenges while the alert is open; they must join this prompt, not stack a second one.
        try await Task.sleep(for: .seconds(3))
        XCTAssertEqual(alerts().count, 1, "exactly one alert while challenges are pending")

        try tap(confirm, on: alert)
        _ = try await waitFor("the page to report ready", seconds: 40) {
            guard let browser = self.browserView(), let recovery = self.findNamed("ConnectionRecoveryView", in: browser) else { return nil as Bool? }
            return recovery.isHidden ? true : nil
        }
        XCTAssertEqual(alerts().count, 0, "no second prompt after trusting")
        let web = try XCTUnwrap(browserView().flatMap { find(WKWebView.self, in: $0) })
        XCTAssertEqual(web.url?.host, URL(string: target)?.host)
        let keys = try XCTUnwrap(plugin.vault.read()["keys"] as? [String: String])
        XCTAssertEqual(keys.count, 1, "the decision is persisted once")
        XCTAssertTrue(keys.keys.first?.hasPrefix("tls:") ?? false)

        _ = try await invoke(plugin, "disconnect", [:])
    }

    @MainActor func testCancelFailsClosedWithAVisibleError() async throws {
        guard !target.isEmpty else { throw XCTSkip("Set VELA_E2E_URL to a VelaTerm desktop with a self-signed certificate") }
        let plugin = try XCTUnwrap(bridgeController().bridge?.plugin(withName: "VelaRemote") as? VelaRemotePlugin)
        let original = try plugin.vault.read()
        defer { try? plugin.vault.write(original) }
        _ = try await start(plugin)

        let alert = try await waitFor("the fingerprint alert", seconds: 15) { self.alerts().first }
        describe(alert)
        let cancel = try XCTUnwrap(alert.actions.first { $0.style == .cancel })
        try tap(cancel, on: alert)

        let texts = try await waitFor("the failure page", seconds: 20) { () -> [String]? in
            guard let browser = self.browserView(), let recovery = self.findNamed("ConnectionRecoveryView", in: browser), !recovery.isHidden else { return nil }
            let found = self.labels(in: recovery)
            return found.contains { $0 == MobileText.get("mobile.native.certificateRejected") } ? found : nil
        }
        NSLog("E2E failure page: %@", texts.joined(separator: " || "))
        // WebKit opens further connections after the refusal; none of them may ask again during this load.
        try await Task.sleep(for: .seconds(3))
        XCTAssertEqual(alerts().count, 0, "no prompt is left behind after cancelling")
        XCTAssertNil(try plugin.vault.read()["keys"].flatMap { ($0 as? [String: String])?.first }, "a cancelled prompt is never persisted")

        _ = try await invoke(plugin, "disconnect", [:])
    }
}
