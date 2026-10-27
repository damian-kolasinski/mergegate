import Foundation
import Networking
import Testing

@testable import Review

struct ReviewParityTests {
    @Test @MainActor func priorityClearsNoteAndAbandonRetryReconcilesClosedItem() async throws {
        let transport = ActionsTransport()
        let model = ReviewActionsModel(
            service: .init(transport: transport, url: URL(string: "http://hub")!), item: "sample#1")
        await model.load()
        #expect(model.priority == "p1" && model.note == "Urgent")
        model.priority = nil
        model.note = "  "
        await model.send(.priority)
        #expect(model.result == "Priority saved")
        #expect(model.snapshot?.item.note == nil)
        await transport.loseAbandonResponse()
        model.reason = "No longer needed"
        await model.send(.abandon)
        #expect(model.error != nil)
        await model.send(.abandon)
        #expect(model.result == "Item already abandoned")
        #expect(await transport.abandons == 1)
    }
    @Test @MainActor func abandonLeavesGithubOnlyAfterGithubLostTheRepository() async throws {
        let transport = ActionsTransport()
        await transport.deleteRepo()
        let model = ReviewActionsModel(
            service: .init(transport: transport, url: URL(string: "http://hub")!), item: "sample#1")
        await model.load()
        await model.send(.abandon)
        #expect(model.error != nil)
        #expect(model.githubUnreachable)
        #expect(model.snapshot?.item.state == "needs_review")
        await model.send(.abandonLeavingGithub)
        #expect(model.result == "Abandoned")
        #expect(!model.githubUnreachable)
        #expect(await transport.leftGithub == [false, true])
    }
    @Test @MainActor func mergeRecoveryRefusesRetryAfterHeldStateMoved() async throws {
        let transport = ActionsTransport()
        await transport.block("held")
        let model = ReviewActionsModel(
            service: .init(transport: transport, url: URL(string: "http://hub")!), item: "sample#1")
        await model.load()
        await transport.block("waiting")
        await model.send(.retryMerge)
        #expect(model.error != nil)
        #expect(await transport.merges == 0)
        #expect(model.snapshot?.blocked?.kind == "waiting")
        await model.send(.cancelMerge)
        #expect(model.result == "Merge cancelled")
        #expect(model.snapshot?.blocked == nil)
    }
    @Test @MainActor func timeTracksInputsSkipsIdleGapsAndKeepsSessionsSeparate() async throws {
        let transport = ActionsTransport()
        let time = ReviewTimeModel(
            service: .init(transport: transport, url: URL(string: "http://hub")!), item: "sample#1")
        let now = Date(timeIntervalSince1970: 0)
        let first = time.start(revision: 1, now: now)
        time.input(now: now.addingTimeInterval(10))
        await time.report()
        let second = time.start(revision: 2, now: now.addingTimeInterval(20))
        await time.stop(session: first, now: now.addingTimeInterval(25))
        time.input(now: now.addingTimeInterval(30))
        await time.stop(session: second, now: now.addingTimeInterval(200))
        let records = await transport.times
        #expect(records.map(\.revision) == [1, 1, 2])
        #expect(records.map(\.ms) == [10_000, 10_000, 10_000])
    }
    @Test func mediaRejectsForeignOriginsAndMarkdownKeepsCodeAndTables() throws {
        let service = ReviewSourceService(
            transport: ActionsTransport(), url: URL(string: "http://hub:7800")!)
        for path in ["https://evil/api/a", "//evil/api/a", "/not-api/file"] {
            #expect(throws: (any Error).self) { try service.mediaURL(path) }
        }
        #expect(try service.mediaURL("/api/attachments/abc").host == "hub")
        #expect(
            MarkdownBlock.parse(
                "# Guide\n\n```swift\nlet a = 1\n```\n\n| A | B |\n| --- | --- |\n| 1 | 2 |") == [
                    .heading("Guide"), .code("let a = 1"), .table([["A", "B"], ["1", "2"]]),
                ])
    }
}
private actor ActionsTransport: HTTPTransport {
    struct Time: Decodable {
        let revision: UInt32
        let ms: UInt64
    }
    var times: [Time] = []
    var state = "needs_review"
    var blocked: String? = nil
    var priority: String? = "p1"
    var note: String? = "Urgent"
    var lost = false
    var abandons = 0
    var repoGone = false
    /// Each abandon's `leave_github`.
    var leftGithub: [Bool] = []
    var merges = 0
    func loseAbandonResponse() { lost = true }
    func deleteRepo() { repoGone = true }
    func block(_ value: String) { blocked = value }
    func get(_ url: URL) async throws -> Data { try data() }
    func post(_ url: URL, body: Data) async throws -> Data {
        switch url.lastPathComponent {
        case "review-time":
            times.append(try JSONDecoder().decode(Time.self, from: body))
            return Data("{}".utf8)
        case "priority":
            let value = try JSONSerialization.jsonObject(with: body) as! [String: Any]
            priority = value["priority"] as? String
            note = value["note"] as? String
            return try itemData()
        case "abandon":
            let value = try JSONSerialization.jsonObject(with: body) as! [String: Any]
            let leaveGithub = value["leave_github"] as? Bool ?? false
            leftGithub.append(leaveGithub)
            if repoGone && !leaveGithub {
                throw HTTPFailure(
                    status: 502, code: "github_no_access",
                    message: "acme/app doesn't exist on GitHub")
            }
            abandons += 1
            state = "abandoned"
            if lost {
                lost = false
                throw URLError(.networkConnectionLost)
            }
            return try itemData()
        case "merge":
            merges += 1
            throw URLError(.unsupportedURL)
        default: throw URLError(.unsupportedURL)
        }
    }
    func delete(_ url: URL) async throws -> Data {
        blocked = nil
        return try itemData()
    }
    func put(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }
    private func item() -> [String: Any] {
        [
            "id": "sample#1", "name": "sample#11", "title": "Feature", "target": "main",
            "state": state,
            "revision": ["number": 1], "priority": priority as Any? ?? NSNull(),
            "note": note as Any? ?? NSNull(),
        ]
    }
    private func itemData() throws -> Data { try JSONSerialization.data(withJSONObject: item()) }
    private func data() throws -> Data {
        var value: [String: Any] = ["item": item(), "files": [], "interdiffs": []]
        if let blocked {
            value["blocked"] = ["kind": blocked, "reason": "Offline", "parent": "sample#9"]
        }
        return try JSONSerialization.data(withJSONObject: value)
    }
}
