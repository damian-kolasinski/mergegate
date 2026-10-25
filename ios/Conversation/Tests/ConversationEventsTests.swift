import ConversationInterface
import Foundation
import Networking
import Testing

@testable import Conversation

struct ConversationEventsTests {
    @Test @MainActor func liveRepliesAndReconnectKeepPinnedScopeAndReadableHistory() async throws {
        let transport = LiveConversationTransport()
        let scope = CommentDraftScope(
            hub: URL(string: "http://my-mac")!, item: "sample#3", name: "sample#93", revision: 1)
        let model = ConversationViewModel(
            scope: scope, path: "checkout.swift", service: .init(transport: transport))
        await model.load()
        #expect(model.threads.first?.replies.isEmpty == true)
        await transport.reply()
        await model.receive(.data(#"{"type":"item.changed","item":"other#1"}"#))
        await model.receive(.data(#"{"type":"highlights.changed"}"#))
        await model.receive(.data("bad frame"))
        #expect(await transport.reads == 1)
        await model.receive(.data(#"{"type":"inbox.changed"}"#))
        #expect(model.threads.first?.replies.first?.body == "Delivery is included. é 🐙")
        #expect(model.threads.first?.status == "Addressed in the next revision")
        #expect(model.scope.revision == 1)
        #expect(model.path == "checkout.swift")
        await model.receive(.disconnected)
        #expect(model.isReconnecting)
        await transport.fail()
        await model.receive(.connected)
        #expect(!model.isReconnecting)
        #expect(model.error != nil)
        #expect(model.threads.first?.replies.count == 1)
        await model.receive(.connected)
        #expect(model.error == nil)
    }
}

private actor LiveConversationTransport: HTTPTransport {
    var replied = false
    var failNext = false
    var reads = 0
    func reply() { replied = true }
    func fail() { failNext = true }
    func get(_ url: URL) async throws -> Data {
        reads += 1
        #expect(url.path == "/api/items/sample#3/conversation")
        #expect(
            URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems == [
                .init(name: "onto", value: "1")
            ])
        if failNext {
            failNext = false
            throw URLError(.timedOut)
        }
        let replies =
            replied
            ? #"{"id":1,"comment":"c_1","author":"agent","status":"addressed","body":"Delivery is included. é 🐙","created_at":"2026-10-25T10:00:00Z"}"#
            : ""
        return Data(
            """
            {"comments":[{"id":"c_1","revision":1,"path":"checkout.swift","side":"new","start_line":3,"line":3,"body":"Include delivery"}],"replies":[\(replies)]}
            """.utf8)
    }
    func put(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }
    func post(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }
    func delete(_ url: URL) async throws -> Data { throw URLError(.unsupportedURL) }
}
