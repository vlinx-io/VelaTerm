// Host-side tests for TrustPromptCoordinator, run by scripts/test-ios-native.sh with plain swiftc on macOS.
// Every case drives the real source file through a scripted presenter; no UIKit, device or simulator involved.
import Foundation

typealias Request = TrustPromptCoordinator.Request

/// Scripted presenter: records every presentation and holds it open until the test answers it.
@MainActor final class Gate {
    private(set) var presented: [Request] = []
    private var open: [CheckedContinuation<Bool, Error>] = []
    private(set) var maxOpen = 0
    var failure: Error?
    struct Unavailable: Error {}
    func present(_ request: Request) async throws -> Bool {
        presented.append(request)
        if let failure { throw failure }
        return try await withCheckedThrowingContinuation { continuation in
            open.append(continuation); maxOpen = max(maxOpen, open.count)
        }
    }
    var openCount: Int { open.count }
    func answer(_ decision: Bool) { let next = open.removeFirst(); next.resume(returning: decision) }
}

var failures = 0
func check(_ condition: Bool, _ label: String) {
    if condition { print("ok   \(label)") } else { failures += 1; print("FAIL \(label)") }
}
/// Yields to the cooperative pool until `condition` holds or about two seconds passed.
@MainActor func settle(_ condition: @escaping () -> Bool) async {
    let deadline = Date().addingTimeInterval(2)
    while !condition(), Date() < deadline { await Task.yield(); try? await Task.sleep(nanoseconds: 1_000_000) }
}
@MainActor func make() -> (TrustPromptCoordinator, Gate) {
    let gate = Gate()
    return (TrustPromptCoordinator { try await gate.present($0) }, gate)
}
let tls = Request(identity: "tls:https://100.117.162.60:8799", fingerprint: "SHA256:aaa", changed: false)
let ssh = Request(identity: "host:22", fingerprint: "SHA256:bbb", changed: false)
let rotated = Request(identity: tls.identity, fingerprint: "SHA256:ccc", changed: true)

// Case 1: N concurrent callers for one identity produce one presentation and N identical answers.
do {
    let (coordinator, gate) = make()
    let tasks = (0..<6).map { _ in Task { await coordinator.decide(tls) } }
    await settle { gate.openCount == 1 }
    check(gate.presented.count == 1, "case1: six concurrent callers, one presentation")
    gate.answer(true)
    var results: [Bool] = []
    for task in tasks { results.append(await task.value) }
    check(results == Array(repeating: true, count: 6), "case1: every caller received true")
    check(gate.presented.count == 1 && !coordinator.isPresenting, "case1: nothing presented afterwards")
    // The coordinator must not cache decisions: remembering trust is the vault's job, and a false is never stored.
    let again = Task { await coordinator.decide(tls) }
    await settle { gate.openCount == 1 }
    check(gate.presented.count == 2, "case1: the same request after an answer prompts again (no decision cache)")
    gate.answer(false)
    check(await again.value == false, "case1: the repeated prompt delivers its own decision")
}

// Case 2: two identities produce two sequential presentations that never overlap; cancel is delivered as false.
do {
    let (coordinator, gate) = make()
    let first = Task { await coordinator.decide(tls) }
    let second = Task { await coordinator.decide(ssh) }
    await settle { gate.openCount == 1 }
    try? await Task.sleep(nanoseconds: 20_000_000)
    check(gate.presented == [tls] && gate.openCount == 1, "case2: only the first identity is on screen")
    gate.answer(true)
    await settle { gate.presented.count == 2 && gate.openCount == 1 }
    check(gate.presented == [tls, ssh], "case2: second identity presented after the first was answered")
    gate.answer(false)
    let results = (await first.value, await second.value)
    check(results == (true, false), "case2: decisions routed to their callers (true, false)")
    check(gate.maxOpen == 1, "case2: prompts never overlapped")
}

// Case 3: a presenter that throws (no presenter available) answers false to every caller.
do {
    let (coordinator, gate) = make()
    gate.failure = Gate.Unavailable()
    let tasks = (0..<3).map { _ in Task { await coordinator.decide(tls) } }
    let queued = Task { await coordinator.decide(ssh) }
    var results: [Bool] = []
    for task in tasks { results.append(await task.value) }
    check(results == [false, false, false], "case3: throwing presenter yields false to all callers")
    check(await queued.value == false, "case3: a request queued behind the failure is answered false, not dropped")
    check(gate.presented.count == 2 && Set(gate.presented) == [tls, ssh], "case3: each distinct request was attempted exactly once")
    gate.failure = nil
    let recovered = Task { await coordinator.decide(ssh) }
    await settle { gate.openCount == 1 }
    gate.answer(true)
    check(await recovered.value == true, "case3: the coordinator keeps working after a failure")
}

// Case 4: generation mismatch answers false, both on entry and when it moves on while the request waits.
do {
    let (coordinator, gate) = make()
    check(await coordinator.decide(tls, isCurrent: { false }) == false, "case4: stale on entry gives false")
    check(gate.presented.isEmpty, "case4: stale on entry presents nothing")
    var generation = 1
    let epoch = generation
    let blocker = Task { await coordinator.decide(ssh) }
    await settle { gate.openCount == 1 }
    let waiting = Task { await coordinator.decide(tls, isCurrent: { generation == epoch }) }
    try? await Task.sleep(nanoseconds: 20_000_000)
    generation += 1
    gate.answer(true)
    check(await waiting.value == false, "case4: generation moved on while queued gives false")
    check(await blocker.value == true && gate.presented == [ssh], "case4: the stale request was never presented")
}

// Case 5: a caller for a second identity arriving while the first prompt is open is presented after the answer.
do {
    let (coordinator, gate) = make()
    let first = Task { await coordinator.decide(tls) }
    await settle { gate.openCount == 1 }
    let late = Task { await coordinator.decide(rotated) }
    try? await Task.sleep(nanoseconds: 20_000_000)
    check(gate.presented == [tls] && gate.openCount == 1, "case5: late request waits while the first prompt is open")
    gate.answer(false)
    await settle { gate.presented.count == 2 }
    check(gate.presented == [tls, rotated] && gate.openCount == 1, "case5: changed fingerprint presented after the first was answered")
    gate.answer(true)
    let results = (await first.value, await late.value)
    check(results == (false, true), "case5: each prompt answers its own callers")
}

// Case 6: a caller for the identity whose prompt is already open joins it instead of queueing a second prompt.
do {
    let (coordinator, gate) = make()
    let first = Task { await coordinator.decide(tls) }
    await settle { gate.openCount == 1 }
    let joiner = Task { await coordinator.decide(tls) }
    try? await Task.sleep(nanoseconds: 20_000_000)
    gate.answer(true)
    let results = (await first.value, await joiner.value)
    check(results == (true, true) && gate.presented.count == 1, "case6: joiner shares the open prompt and its decision")
}

print(failures == 0 ? "All TrustPromptCoordinator cases passed" : "\(failures) TrustPromptCoordinator case(s) failed")
exit(failures == 0 ? 0 : 1)
