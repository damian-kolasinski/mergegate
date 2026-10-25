import Foundation

public struct ConnectedHub: Codable, Equatable, Sendable {
    public let url: URL
    public let deviceName: String

    public init(url: URL, deviceName: String) {
        self.url = url
        self.deviceName = deviceName
    }
}
