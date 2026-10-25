import ConversationInterface
import DiffDocumentInterface
import Foundation
import Networking
import Testing

@testable import Conversation

struct ConversationTests {
    @Test @MainActor func readsPinnedThreadsAndRetriesWithoutDiscardingLastResponse() async throws {
        let transport = ThreadTransport()
        let scope = CommentDraftScope(
            hub: URL(string: "http://my-mac:7800")!, item: "sample#3", name: "sample#93",
            revision: 2)
        let model = ConversationViewModel(scope: scope, service: .init(transport: transport))
        await transport.fail()
        await model.load()
        #expect(model.error != nil)
        #expect(model.snapshot == nil)
        await model.load()
        #expect(model.error == nil)
        #expect(model.threads.map(\.id) == ["c_1", "c_2", "c_3", "c_4", "c_5"])
        #expect(model.threads[0].status == "Operator replied · Question")
        #expect(model.threads[0].replies.map(\.body) == ["Which clients?", "The receipt client."])
        #expect(model.threads[1].status == "Addressed in r2")
        #expect(model.threads[2].location == "Whole item")
        #expect(model.threads[2].status == "Needs your vote")
        #expect(model.threads[3].status == "Rejection accepted")
        #expect(model.threads[4].placementLabel == "From later revision r3")
        await transport.fail()
        await model.load()
        #expect(model.error != nil)
        #expect(model.threads.count == 5)
        #expect(!model.loading)
    }

    @Test func placementsUsePinnedCoordinatesAndNeverAttachOutdatedOrFutureComments() throws {
        let snapshot = try JSONDecoder().decode(ConversationSnapshot.self, from: threadWire)
        let selected = SourceAnchor(side: .new, startLine: 8, endLine: 9, quote: "wrapped é 🐙")
        let threads = snapshot.threads(path: "checkout.swift", anchors: [selected], revision: 2)
        #expect(threads.map(\.id) == ["c_1"])
        #expect(threads[0].location == "After · Lines 8–10")
        #expect(threads[0].placementLabel == "Moved from r1")
        #expect(snapshot.threads(path: "checkout.swift", anchors: [selected], revision: 1).isEmpty)
        let old = SourceAnchor(side: .old, startLine: 3, endLine: 3, quote: "")
        #expect(snapshot.threads(path: "checkout.swift", anchors: [old], revision: 2).isEmpty)
        #expect(snapshot.threads(path: "other.swift", anchors: [selected], revision: 2).isEmpty)
        let outdated = snapshot.threads(revision: 2)[1]
        #expect(outdated.sourceRange == nil)
        #expect(outdated.placementLabel == "Outdated · Not on this diff")
    }

    @Test func changedPlacementsAndPendingAddressedRepliesRemainExplicit() throws {
        let wire = String(decoding: threadWire, as: UTF8.self)
            .replacingOccurrences(of: "\"moved\"", with: "\"changed\"")
            .replacingOccurrences(of: "\"addressed_in\":2", with: "\"addressed_in\":null")
        let snapshot = try JSONDecoder().decode(ConversationSnapshot.self, from: Data(wire.utf8))
        let threads = snapshot.threads(revision: 2)
        #expect(threads[0].sourceRange == 8...10)
        #expect(threads[0].placementLabel == "Source changed since r1")
        #expect(threads[1].status == "Addressed in the next revision")
    }

    @Test func overrideAfterAcceptingRejectionReopensTheThread() throws {
        let snapshot = try JSONDecoder().decode(ConversationSnapshot.self, from: threadWire)
        let comment = snapshot.comments[3]
        let override = try JSONDecoder().decode(
            ThreadReply.self,
            from: Data(
                """
                {"id":5,"comment":"c_4","author":"operator","status":null,"body":"Still needed.","overrides":true,"created_at":"2026-10-25T10:04:00Z"}
                """.utf8))
        let thread = ConversationThread(
            comment: comment, replies: [snapshot.replies[3], override], revision: 2)
        #expect(thread.status == "Override sent")
    }

    @Test func unplacedExternalCommentRetainsItsFileWithoutInventingAnAnchor() throws {
        let wire = String(decoding: threadWire, as: UTF8.self).replacingOccurrences(
            of: "\"author\":\"reviewer\",\"pending\":true",
            with: "\"author\":\"reviewer\",\"path\":\"policy.txt\",\"pending\":true")
        let snapshot = try JSONDecoder().decode(ConversationSnapshot.self, from: Data(wire.utf8))
        let thread = try #require(snapshot.threads(path: "policy.txt", revision: 2).first)
        #expect(thread.comment.filePath == "policy.txt")
        #expect(thread.location == "File comment · No source anchor")
        #expect(thread.sourceRange == nil)
        #expect(
            snapshot.threads(
                path: "policy.txt",
                anchors: [
                    .init(side: .new, startLine: 1, endLine: 1, quote: "")
                ], revision: 2
            ).isEmpty)
    }
}

private actor ThreadTransport: HTTPTransport {
    var failing = false
    func fail() { failing = true }
    func delete(_ url: URL) async throws -> Data { throw URLError(.unsupportedURL) }

    func post(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }

    func put(_ url: URL, body: Data) throws -> Data { throw URLError(.unsupportedURL) }
    func get(_ url: URL) throws -> Data {
        #expect(url.path == "/api/items/sample#3/conversation")
        #expect(url.fragment == nil)
        #expect(
            URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems == [
                URLQueryItem(name: "onto", value: "2")
            ])
        if failing {
            failing = false
            throw URLError(.timedOut)
        }
        return threadWire
    }
}

private let threadWire = Data(
    """
    {"comments":[
      {"id":"c_1","revision":1,"path":"checkout.swift","side":"new","start_line":3,"line":5,"body":"Keep Unicode é 🐙","quote":"let total = subtotal","placement":{"kind":"moved","start_line":8,"line":10}},
      {"id":"c_2","revision":1,"path":"checkout.swift","side":"old","start_line":3,"line":3,"body":"Include delivery","placement":{"kind":"outdated"}},
      {"id":"c_3","revision":2,"body":"Review the whole change","external":{"author":"reviewer","pending":true,"vote":null,"agreed_reply":null}},
      {"id":"c_4","revision":2,"body":"Use an extra dependency","external":{"author":"reviewer","pending":false,"vote":"up","agreed_reply":4}},
      {"id":"c_5","revision":3,"path":"checkout.swift","side":"new","start_line":8,"line":8,"body":"Future anchor"}
    ],"replies":[
      {"id":1,"comment":"c_1","author":"agent","status":"question","body":"Which clients?","created_at":"2026-10-25T10:00:00Z"},
      {"id":2,"comment":"c_2","author":"agent","status":"addressed","body":"","addressed_in":2,"created_at":"2026-10-25T10:01:00Z"},
      {"id":3,"comment":"c_1","author":"operator","status":null,"body":"The receipt client.","created_at":"2026-10-25T10:02:00Z"},
      {"id":4,"comment":"c_4","author":"agent","status":"rejected","body":"Already covered.","created_at":"2026-10-25T10:03:00Z"}
    ],"drafts":[{"id":"unsent","revision":2,"body":"Unsent comment"}],"draft_replies":[{"id":99,"comment":"c_1","author":"operator","body":"Unsent reply","created_at":"2026-10-25T10:00:00Z"}]}
    """.utf8)
