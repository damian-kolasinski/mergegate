import Foundation
import HubConnectionInterface
import Networking

struct HubConnectionWebService: Sendable {
    let transport: any HTTPTransport

    func connect(to url: URL) async throws -> ConnectedHub {
        let data = try await transport.get(url.appending(path: "api/whoami"))
        let identity = try JSONDecoder().decode(Identity.self, from: data)
        guard identity.via == "tailnet", let device = identity.device,
            ["owner", "allowed"].contains(device.trust)
        else {
            throw ConnectionError()
        }
        return ConnectedHub(url: url, deviceName: device.name)
    }
}

// The feature's consumed subset of crates/api/src/device.rs.
private struct Identity: Decodable {
    struct Device: Decodable {
        let name: String
        let trust: String
    }
    let via: String
    let device: Device?
}

private struct ConnectionError: LocalizedError {
    var errorDescription: String? {
        "Use the hub’s Tailscale address. The hub must recognize this device through Tailscale."
    }
}
