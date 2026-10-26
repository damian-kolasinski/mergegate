import Foundation
import Networking
import Testing

@testable import Inbox

struct TriageTests {
    @Test
    func pendingDecisionsFollowLatestSentReplyAndExternalAgreement() throws {
        let data = Data("""
        {"item":{"revision":{"number":2}},"files":[
          {"tier":"review","viewed":false},{"tier":"review","viewed":true},{"tier":"trusted","viewed":false}],
         "comments":[
          {"id":"q","body":"Keep the old format?","path":"a.swift"},
          {"id":"answered","body":"Answered","path":null},
          {"id":"rejected","body":"Use a cache","external":{"pending":false,"summary":false,"lapsed":false,"agreed_reply":null}},
          {"id":"agreed","body":"Accepted rejection","external":{"pending":false,"summary":false,"lapsed":false,"agreed_reply":5}},
          {"id":"operator","body":"Operator rejection"},
          {"id":"vote","body":"Check bounds","external":{"pending":true,"summary":false,"lapsed":false}}],
         "replies":[
          {"id":1,"comment":"q","author":"agent","status":"question","body":"Which clients still need it?"},
          {"id":2,"comment":"answered","author":"agent","status":"question","body":"Which?"},
          {"id":3,"comment":"answered","author":"operator","status":null,"body":"Both"},
          {"id":4,"comment":"rejected","author":"agent","status":"rejected","body":"The response must stay fresh."},
          {"id":5,"comment":"agreed","author":"agent","status":"rejected","body":"Agreed"},
          {"id":6,"comment":"operator","author":"agent","status":"rejected","body":"No"}],
         "draft_replies":[{"comment":"q","author":"operator","body":"Unsent answer"}]}
        """.utf8)
        let triage = try JSONDecoder().decode(InboxTriage.self, from: data)
        #expect(triage.decisions.map(\.id) == ["q", "rejected", "vote"])
        #expect(triage.decisions.map(\.kind) == ["Question", "Disagreement", "Vote needed"])
        #expect(triage.decisions.first?.comment == "Keep the old format?")
        #expect(triage.decisions.first?.response == "Which clients still need it?")
    }

    @Test
    func sectionsFollowTheWebAndKeepHubOrder() throws {
        let snapshot = try JSONDecoder().decode(InboxSnapshot.self, from: Data("""
        {"needs_you":[
          {"id":"alpha#1","name":"alpha#71","title":"Review","state":"needs_review","revision":1,"review_files":2,"additions":1,"deletions":0},
          {"id":"beta#2","name":"beta#18","title":"Question","state":"changes_requested","revision":2,"review_files":1,"questions":1,"additions":1,"deletions":0}],
         "with_agents":[{"id":"beta#4","name":"beta#19","title":"Working","state":"changes_requested","revision":1,"review_files":1,"additions":1,"deletions":0}],
         "merging":[],
         "closed":[{"id":"alpha#5","name":"alpha#70","title":"Done","state":"merged","revision":1,"review_files":1,"additions":1,"deletions":0}]}
        """.utf8))
        #expect(snapshot.projects == ["alpha", "beta"])
        let all = snapshot.groups(project: nil)
        #expect(all.map(\.section) == [.needsYou, .withAgents])
        #expect(all[0].items.map(\.id) == ["alpha#1", "beta#2"])
        #expect(snapshot.closed(project: nil).map(\.id) == ["alpha#5"])
        // Needs you stays, empty, so it can say why; With agents has nothing for alpha.
        let alpha = snapshot.groups(project: "alpha")
        #expect(alpha.map(\.section) == [.needsYou])
        #expect(snapshot.groups(project: "beta").first?.items.map(\.id) == ["beta#2"])
        #expect(snapshot.groups(project: "gamma").map(\.section) == [.needsYou])
        #expect(snapshot.groups(project: "gamma").first?.items.isEmpty == true)
    }

    @Test @MainActor
    func staleDetailDoesNotReplaceCurrentRevisionAndFailureCanRetry() async throws {
        let transport = TriageTransport()
        let model = InboxViewModel(service: .init(transport: transport, url: URL(string: "http://my-mac:7800")!))
        await model.refresh()
        let item = try #require(model.snapshot?.needsYou.first)
        await model.loadTriage(item)
        #expect(model.triage[item.id] == nil)
        #expect(model.triageErrors[item.id]?.contains("newer revision") == true)
        await transport.setRevision(1)
        await model.loadTriage(item)
        #expect(model.triage[item.id] != nil)
        #expect(model.triageErrors[item.id] == nil)
        model.project = "removed-project"
        await model.refresh()
        #expect(model.project == nil)
        #expect(model.triage.isEmpty)
    }
}

private actor TriageTransport: HTTPTransport {
    func delete(_ url: URL) async throws -> Data { throw URLError(.unsupportedURL) }

    func post(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }

    func put(_ url: URL, body: Data) async throws -> Data {
        throw URLError(.unsupportedURL)
    }

    var revision = 2
    func setRevision(_ value: Int) { revision = value }
    func get(_ url: URL) async throws -> Data {
        if url.path == "/api/inbox" {
            return Data("""
            {"needs_you":[{"id":"sample#1","name":"sample#9","title":"Test","state":"changes_requested","revision":1,"questions":1,"review_files":0,"additions":0,"deletions":0}],"with_agents":[],"merging":[],"closed":[]}
            """.utf8)
        }
        #expect(url.path == "/api/items/sample#1")
        #expect(url.fragment == nil)
        return Data("""
        {"item":{"revision":{"number":\(revision)}},"comments":[],"replies":[]}
        """.utf8)
    }
}
