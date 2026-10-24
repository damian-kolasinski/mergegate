import Foundation
import Testing

@testable import Networking

struct TransportTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["MG_IOS_TEST_HUB_URL"]?.hasPrefix("http") == true))
    func realHubReturnsJSONThroughTailnet() async throws {
        let address = ProcessInfo.processInfo.environment["MG_IOS_TEST_HUB_URL"] ?? ""
        let url = try #require(URL(string: address))
        let data = try await URLSessionTransport().get(url.appending(path: "api/whoami"))
        let identity = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        #expect(identity?["via"] as? String == "tailnet")
        #expect(identity?["device"] != nil)
    }
}
