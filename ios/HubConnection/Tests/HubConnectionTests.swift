import Foundation
import HubConnectionInterface
import Networking
import Testing

@testable import HubConnection

struct HubConnectionTests {
    @Test(arguments: [
        "http://my-mac:7800", "http://100.93.225.125:7800", "http://my-mac.tail123.ts.net:7800",
        "http://[fd7a:115c:a1e0::1]:7800",
    ])
    func acceptsHubAddress(_ address: String) throws {
        #expect(try HubAddress(address).url.host != nil)
    }

    @Test(arguments: [
        "", "my-mac:7800", "ftp://my-mac", "http://100.63.1.1", "http://192.168.1.1", "http://example.com",
        "https://100.64.1.1.example.com", "http://my-mac/api", "http://user:pass@my-mac", "http://my-mac?x=1",
        "http://my-mac#fragment", "http://my-mac:0", "http://-my-mac",
    ])
    func rejectsInvalidAddress(_ address: String) {
        #expect(throws: (any Error).self) { try HubAddress(address) }
    }

    @Test @MainActor
    func retainsOnlySuccessfullyTrustedHub() async throws {
        let name = "HubConnectionTests-\(UUID())"
        let defaults = try #require(UserDefaults(suiteName: name))
        defer { defaults.removePersistentDomain(forName: name) }
        let selection = HubSelection(defaults: defaults)
        let service = HubConnectionWebService(transport: IdentityTransport(via: "tailnet"))
        let model = HubConnectionViewModel(interactor: .init(service: service, selection: selection))
        model.address = "http://my-mac:7800"
        await model.connect()
        #expect(selection.url?.absoluteString == model.address)
        let reopened = HubConnectionViewModel(interactor: .init(service: service, selection: selection))
        await reopened.restore()
        guard case .connected(let hub) = reopened.state else {
            Issue.record("Hub was not reopened")
            return
        }
        #expect(hub.deviceName == "phone")
        reopened.forget()
        #expect(selection.url == nil)
        #expect(reopened.address.isEmpty)
    }

    @Test @MainActor
    func trustedHubRestoresWithoutAWorkingNetworkAndForgetClearsIdentity() async throws {
        let name = "HubOfflineTests-\(UUID())"
        let defaults = try #require(UserDefaults(suiteName: name))
        defer { defaults.removePersistentDomain(forName: name) }
        let selection = HubSelection(defaults: defaults)
        selection.save(ConnectedHub(url: URL(string: "http://my-mac")!, deviceName: "phone"))
        let model = HubConnectionViewModel(
            interactor: .init(
                service: .init(transport: NoNetworkIdentity()), selection: selection))
        await model.restore()
        guard case .connected(let hub) = model.state else {
            Issue.record("Expected offline hub")
            return
        }
        #expect(hub.deviceName == "phone")
        model.forget()
        #expect(selection.hub == nil && selection.url == nil)
    }

    @Test @MainActor
    func refusesLoopbackIdentityWithoutSaving() async throws {
        let name = "HubConnectionTests-\(UUID())"
        let defaults = try #require(UserDefaults(suiteName: name))
        defer { defaults.removePersistentDomain(forName: name) }
        let selection = HubSelection(defaults: defaults)
        let model = HubConnectionViewModel(
            interactor: .init(
                service: .init(transport: IdentityTransport(via: "loopback")), selection: selection))
        model.address = "http://my-mac:7800"
        await model.connect()
        #expect(model.error != nil)
        #expect(selection.url == nil)
    }
}

private struct IdentityTransport: HTTPTransport {
    func delete(_ url: URL) async throws -> Data { throw URLError(.unsupportedURL) }

    func post(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }

    func put(_ url: URL, body: Data) async throws -> Data {
        throw URLError(.unsupportedURL)
    }

    let via: String
    func get(_ url: URL) async throws -> Data {
        #expect(url.path == "/api/whoami")
        return Data("{\"via\":\"\(via)\",\"device\":{\"name\":\"phone\",\"trust\":\"owner\"}}".utf8)
    }
}

private struct NoNetworkIdentity: HTTPTransport {
    func get(_ url: URL) async throws -> Data {
        Issue.record("Restoring the saved hub must not require a network request")
        throw URLError(.notConnectedToInternet)
    }
    func put(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }
    func post(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }
    func delete(_ url: URL) async throws -> Data { throw URLError(.unsupportedURL) }
}
