import Foundation
import Networking
import Testing

@testable import ProjectRules

struct RulesTests {
    @Test @MainActor func decisionsPreservePreviewAndRecoverFromFailure() async throws {
        let transport = RulesTransport()
        let model = RulesViewModel(
            service: .init(hub: URL(string: "http://hub")!, project: "sample", transport: transport)
        )
        await model.load()
        #expect(
            model.snapshot?.pending.first?.preview?.summary
                == "Would have hidden 2 files in 1 of 3 recent items.")
        await transport.fail()
        await model.load()
        #expect(model.error != nil)
        #expect(model.snapshot?.pending.count == 1)
        await model.decide(.proposal("R1", approve: true))
        #expect(model.error == nil)
        #expect(model.snapshot?.pending.isEmpty == true)
        #expect(model.snapshot?.rules.first?.id == "R1")
        #expect(await transport.writes == ["/api/projects/sample/rules/proposals/R1/approve"])
    }
    @Test func rejectsRulesForAnotherProjectAndEncodesComponents() async throws {
        let transport = RulesTransport()
        let service = RulesWebService(
            hub: URL(string: "http://hub")!, project: "other/#?", transport: transport)
        await #expect(throws: (any Error).self) {
            try await service.decide(.proposal("R/1", approve: false))
        }
        let path = try #require(await transport.urls.last)
        #expect(path.absoluteString.contains("other%2F%23%3F"))
        #expect(path.absoluteString.contains("R%2F1/reject"))
        #expect(path.fragment == nil)
    }
}
private actor RulesTransport: HTTPTransport {
    var writes: [String] = []
    var urls: [URL] = []
    var failNext = false
    var applied = false
    func fail() { failNext = true }
    func get(_ url: URL) async throws -> Data {
        if failNext {
            failNext = false
            throw URLError(.timedOut)
        }
        return data()
    }
    func post(_ url: URL, body: Data) async throws -> Data {
        writes.append(url.path)
        urls.append(url)
        applied = true
        return data()
    }
    func put(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }
    func delete(_ url: URL) async throws -> Data { throw URLError(.unsupportedURL) }
    private func data() -> Data {
        let rule = #"{"id":"R1","tier":"trusted","summary":"*.lock","reason":"Generated"}"#
        let pending =
            applied
            ? "[]"
            : "[{\"rule\":\(rule),\"preview\":{\"items\":3,\"hidden\":[{\"item\":\"sample#1\",\"name\":\"sample#8\",\"title\":\"One\",\"path\":\"a.lock\",\"rule\":\"R1\"},{\"item\":\"sample#1\",\"name\":\"sample#8\",\"title\":\"One\",\"path\":\"b.lock\",\"rule\":\"R1\"}]}}]"
        return Data(
            "{\"project\":\"sample\",\"location\":\"rules.toml\",\"rules\":\(applied ? "[\(rule)]" : "[]"),\"pending\":\(pending)}"
                .utf8)
    }
}
