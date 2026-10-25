import DiffDocumentInterface
import Foundation
import Networking
import SwiftData
import Testing

@testable import Review

@MainActor
struct ReviewOfflineTests {
    @Test func reopensExactComparisonFileContextAndOffsetThenPinsAcrossReconnect() async throws {
        let directory = URL.temporaryDirectory.appending(component: UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let cache = ReviewCache(directory: directory)
        let store = try ReviewReadingStore(
            configuration: ModelConfiguration(isStoredInMemoryOnly: true))
        let hub = OfflineReviewHub()
        let url = URL(string: "http://my-mac:7800")!
        let service = ReviewWebService(transport: hub, url: url, cache: cache)
        let first = ReviewViewModel(service: service, item: "sample#1", readingStore: store)
        await first.load()
        await first.compare(since: 1)
        first.selectedPath = "second.swift"
        first.readingOffset = 713
        first.contextLoaded = true
        first.revealed = [0: 20, 1: 8]
        first.rememberPosition()
        await hub.disconnect()
        let reopened = ReviewViewModel(service: service, item: "sample#1", readingStore: store)
        await reopened.load()
        guard case .loaded(let saved) = reopened.state else {
            Issue.record("Expected saved review")
            return
        }
        #expect(saved.item.revision.number == 2 && saved.since == 1)
        #expect(reopened.selectedPath == "second.swift")
        #expect(reopened.readingOffset == 713)
        #expect(reopened.contextLoaded && reopened.revealed == [0: 20, 1: 8])
        #expect(reopened.cachedAt != nil)
        await hub.reconnect(revision: 3)
        await reopened.receive(.connected)
        #expect(reopened.latestRevision == 3 && reopened.cachedAt == nil)
        guard case .loaded(let pinned) = reopened.state else { return }
        #expect(pinned.item.revision.number == 2 && pinned.since == 1)
        #expect(reopened.readingOffset == 713)
        await reopened.load(allowCached: false)
        guard case .loaded(let latest) = reopened.state else { return }
        #expect(latest.item.revision.number == 3)
        #expect(reopened.readingOffset == 0 && !reopened.contextLoaded && reopened.revealed.isEmpty)
    }

    @Test func onlineRelaunchRetainsChosenComparisonAndNewRevisionResetsSameFile() async throws {
        let store = try ReviewReadingStore(configuration: ModelConfiguration(isStoredInMemoryOnly: true))
        let hub = OfflineReviewHub()
        let service = ReviewWebService(transport: hub, url: URL(string: "http://my-mac")!)
        let first = ReviewViewModel(service: service, item: "sample#1", readingStore: store)
        await first.load()
        await first.compare(since: nil)
        first.readingOffset = 713
        first.contextLoaded = true
        first.revealed = [0: 20]
        first.rememberPosition()
        let reopened = ReviewViewModel(service: service, item: "sample#1", readingStore: store)
        await reopened.load()
        guard case .loaded(let snapshot) = reopened.state else { return }
        #expect(snapshot.since == nil && reopened.readingOffset == 713 && reopened.contextLoaded)
        await hub.reconnect(revision: 3)
        await reopened.load(allowCached: false)
        #expect(reopened.selectedPath == "first.swift")
        #expect(reopened.readingOffset == 0 && !reopened.contextLoaded && reopened.revealed.isEmpty)
    }

    @Test func restoresImmutableSourceAndHighlightTuplesWithNoHub() async throws {
        let directory = URL.temporaryDirectory.appending(component: UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let hub = OfflineReviewHub()
        let web = ReviewWebService(
            transport: hub, url: URL(string: "http://my-mac")!, cache: ReviewCache(directory: directory))
        let snapshot = try await web.load(item: "sample#1")
        let file = try #require(snapshot.files.first)
        guard case .text(let hunks) = file.content else { return }
        let source = ReviewSourceService(transport: hub, url: web.url, cache: web.cache)
        let highlights = ReviewHighlightService(web: web, events: ServerEvents())
        let before = try await source.load(project: "sample", file: file, hunks: hunks)
        let painting = try await highlights.load(item: "sample#1", revision: 2, file: file)
        #expect(painting.new?.first?.first?.tag == "keyword")
        await hub.disconnect()
        let after = try await source.load(project: "sample", file: file, hunks: hunks)
        let offline = try await highlights.load(item: "sample#1", revision: 2, file: file)
        #expect(before.gaps.map(\.lines) == after.gaps.map(\.lines))
        #expect(offline.new == painting.new)
        #expect(offline.paint(hunks).first?.lines.last?.spans.first?.tag == "keyword")
        #expect(offline.paint(hunks).first?.lines.last?.text == "let total = 2 // é 🐙")
    }

    @Test func missingCorruptOrOtherHubCacheCannotMasqueradeAsThisReview() async throws {
        let directory = URL.temporaryDirectory.appending(component: UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let cache = ReviewCache(directory: directory)
        let hub = OfflineReviewHub()
        let service = ReviewWebService(
            transport: hub, url: URL(string: "http://one-mac")!, cache: cache)
        _ = try await service.load(item: "sample#1")
        let position = ReviewReadingPosition(revision: 2, since: nil, path: "first.swift")
        #expect(await service.cached(item: "sample#1", position: position) != nil)
        let other = ReviewWebService(
            transport: hub, url: URL(string: "http://other-mac")!, cache: cache)
        #expect(await other.cached(item: "sample#1", position: position) == nil)
        #expect(await service.cached(item: "other#1", position: position) == nil)
        await cache.save(
            Data("broken".utf8),
            key: cache.snapshotKey(hub: service.url, item: "sample#1", revision: 2, since: nil))
        #expect(await service.cached(item: "sample#1", position: position) == nil)
    }

    @Test func authorizationFailureDoesNotFallbackToSavedReview() async throws {
        let directory = URL.temporaryDirectory.appending(component: UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try ReviewReadingStore(
            configuration: ModelConfiguration(isStoredInMemoryOnly: true))
        let hub = OfflineReviewHub()
        let service = ReviewWebService(
            transport: hub, url: URL(string: "http://my-mac")!, cache: ReviewCache(directory: directory))
        let model = ReviewViewModel(service: service, item: "sample#1", readingStore: store)
        await model.load()
        await hub.deny()
        let reopened = ReviewViewModel(service: service, item: "sample#1", readingStore: store)
        await reopened.load()
        if case .failed = reopened.state {
        } else {
            Issue.record("Authorization must remain authoritative")
        }
        #expect(reopened.cachedAt == nil)
    }
}

private actor OfflineReviewHub: HTTPTransport {
    var offline = false
    var denied = false
    var revision = 2
    func disconnect() { offline = true }
    func reconnect(revision: UInt32) {
        offline = false
        self.revision = Int(revision)
    }
    func deny() { denied = true }
    func get(_ url: URL) async throws -> Data {
        if offline { throw URLError(.notConnectedToInternet) }
        if denied {
            throw HTTPFailure(status: 403, code: "forbidden", message: "Device no longer trusted")
        }
        if url.path.contains("/highlights/") { return Data(#"{"lines":[[[0,3,"keyword"]]]}"#.utf8) }
        if url.path.contains("/blobs/") {
            return Data(
                (url.lastPathComponent == "old" ? "let total = 1\n" : "let total = 2 // é 🐙\n").utf8)
        }
        let since =
            URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems?.first?.value ?? "all"
        return Data(
            """
            {"item":{"id":"sample#1","name":"sample#1","title":"Totals","target":"main","state":"needs_review","revision":{"number":\(revision)}},
            "since":\(since == "all" ? "null" : since),"interdiffs":[1],"last_reviewed":1,"files":[
            {"path":"first.swift","grammar":"swift","old_blob":"old","new_blob":"new","tier":"review","viewed":false,
            "content":{"kind":"text","hunks":[{"old_start":1,"old_lines":1,"new_start":1,"new_lines":1,"lines":["-let total = 1","+let total = 2 // é 🐙"]}]}},
            {"path":"second.swift","tier":"review","viewed":false,"content":{"kind":"text","hunks":[]}}]}
            """.utf8)
    }
    func put(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }
    func post(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }
    func delete(_ url: URL) async throws -> Data { throw URLError(.unsupportedURL) }
}
