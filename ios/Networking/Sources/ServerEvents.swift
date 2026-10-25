import Foundation

public enum ServerEvent: Sendable {
    case connected
    case disconnected
    case data(String)
}

// One physical stream per hub, shared by foreground feature consumers.
public actor ServerEvents {
    public static let shared = ServerEvents()
    private var subscribers: [URL: [UUID: AsyncStream<ServerEvent>.Continuation]] = [:]
    private var connections: [URL: Task<Void, Never>] = [:]
    private var connected: [URL: Bool] = [:]

    public init() {}

    public func subscribe(_ url: URL) -> AsyncStream<ServerEvent> {
        let id = UUID()
        let (stream, continuation) = AsyncStream<ServerEvent>.makeStream(
            bufferingPolicy: .bufferingNewest(64))
        subscribers[url, default: [:]][id] = continuation
        if let connected = connected[url] {
            continuation.yield(connected ? .connected : .disconnected)
        }
        continuation.onTermination = { [weak self] _ in Task { await self?.remove(id, url: url) } }
        if connections[url] == nil {
            connections[url] = Task { await self.receive(url) }
        }
        return stream
    }

    private func remove(_ id: UUID, url: URL) {
        subscribers[url]?[id] = nil
        if subscribers[url]?.isEmpty == true {
            subscribers[url] = nil
            connected[url] = nil
            connections.removeValue(forKey: url)?.cancel()
        }
    }

    private func broadcast(_ event: ServerEvent, url: URL) {
        switch event {
        case .connected: connected[url] = true
        case .disconnected: connected[url] = false
        case .data: break
        }
        for continuation in subscribers[url]?.values ?? [:].values { continuation.yield(event) }
    }

    private func receive(_ url: URL) async {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 45
        configuration.timeoutIntervalForResource = 24 * 60 * 60
        let session = URLSession(
            configuration: configuration, delegate: URLSessionTransport(), delegateQueue: nil)
        defer { session.invalidateAndCancel() }
        while !Task.isCancelled {
            do {
                var request = URLRequest(url: url)
                request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
                request.setValue("mergegate-ios", forHTTPHeaderField: "X-Mergegate-Client")
                let (bytes, response) = try await session.bytes(for: request)
                try Task.checkCancellation()
                guard let response = response as? HTTPURLResponse, response.statusCode == 200,
                    response.mimeType == "text/event-stream"
                else { throw URLError(.badServerResponse) }
                broadcast(.connected, url: url)
                var frame = ServerEventDecoder()
                for try await byte in bytes {
                    try Task.checkCancellation()
                    if let data = frame.append(byte) { broadcast(.data(data), url: url) }
                }
            } catch { /* Reconnect invalidates feature state through connected, including after missed frames. */
            }
            guard !Task.isCancelled else { return }
            broadcast(.disconnected, url: url)
            do { try await Task.sleep(for: .seconds(2)) } catch { return }
        }
    }
}
