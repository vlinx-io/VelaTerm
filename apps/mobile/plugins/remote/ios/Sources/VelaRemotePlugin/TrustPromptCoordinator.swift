import Foundation

/// Serializes fingerprint prompts so that at most one is shown at a time.
///
/// Concurrent requests for the same identity and fingerprint share one presentation and receive the
/// same decision; distinct requests are queued and presented one after another. The presenter is
/// injected so this type stays free of UIKit and can be exercised from a plain `swiftc` test on macOS.
/// Every caller is answered exactly once: a presenter that throws answers `false` to all of its waiters.
@MainActor final class TrustPromptCoordinator {
    struct Request: Hashable, Sendable {
        let identity: String
        let fingerprint: String
        let changed: Bool
    }
    /// Shows one prompt and returns the user's decision; throws when nothing can be presented.
    typealias Presenter = @Sendable @MainActor (Request) async throws -> Bool

    private struct Waiter {
        let continuation: CheckedContinuation<Bool, Never>
        let isCurrent: () -> Bool
    }

    private nonisolated let presenter: Presenter
    private var waiters: [Request: [Waiter]] = [:]
    private var queue: [Request] = []
    private var active: Request?

    nonisolated init(presenter: @escaping Presenter) { self.presenter = presenter }

    /// Whether a prompt is on screen right now (exposed for tests).
    var isPresenting: Bool { active != nil }

    /// Resolves `request` to a decision. `isCurrent` is evaluated on entry and again right before the
    /// prompt is shown; a stale caller (for example after the connection generation moved on) gets `false`.
    func decide(_ request: Request, isCurrent: @escaping () -> Bool = { true }) async -> Bool {
        guard isCurrent() else { return false }
        return await withCheckedContinuation { continuation in
            waiters[request, default: []].append(Waiter(continuation: continuation, isCurrent: isCurrent))
            if active == nil { active = request; Task { await self.run() } }
            else if active != request, !queue.contains(request) { queue.append(request) }
        }
    }

    private func run() async {
        while let request = active {
            // Answer callers whose generation moved on without presenting anything for them.
            let (live, stale) = (waiters[request] ?? []).reduce(into: ([Waiter](), [Waiter]())) { result, waiter in
                if waiter.isCurrent() { result.0.append(waiter) } else { result.1.append(waiter) }
            }
            waiters[request] = live.isEmpty ? nil : live
            stale.forEach { $0.continuation.resume(returning: false) }
            var decision = false
            if !live.isEmpty { decision = (try? await presenter(request)) ?? false }
            // Callers that joined while the prompt was open share the same decision.
            for waiter in waiters.removeValue(forKey: request) ?? [] { waiter.continuation.resume(returning: decision) }
            active = queue.isEmpty ? nil : queue.removeFirst()
        }
    }
}
