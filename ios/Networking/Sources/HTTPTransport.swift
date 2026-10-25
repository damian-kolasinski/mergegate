import Foundation

public protocol HTTPTransport: Sendable {
    func get(_ url: URL) async throws -> Data
    func delete(_ url: URL) async throws -> Data
    func post(_ url: URL, body: Data) async throws -> Data
    func put(_ url: URL, body: Data) async throws -> Data
}

public struct HTTPFailure: Error, LocalizedError, Sendable {
    public let status: Int
    public let code: String
    public let message: String
    public let hint: String?

    public init(status: Int, code: String, message: String, hint: String? = nil) {
        self.status = status
        self.code = code
        self.message = message
        self.hint = hint
    }

    public var errorDescription: String? {
        [message, hint].compactMap { $0 }.joined(separator: "\n")
    }
}

public final class URLSessionTransport: NSObject, HTTPTransport, URLSessionTaskDelegate, Sendable {
    public override init() {}

    public func get(_ url: URL) async throws -> Data {
        try await send(url, method: "GET", body: nil)
    }

    public func delete(_ url: URL) async throws -> Data {
        try await send(url, method: "DELETE", body: nil)
    }

    public func post(_ url: URL, body: Data) async throws -> Data {
        try await send(url, method: "POST", body: body)
    }

    public func put(_ url: URL, body: Data) async throws -> Data {
        try await send(url, method: "PUT", body: body)
    }

    private func send(_ url: URL, method: String, body: Data?) async throws -> Data {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 15
        configuration.timeoutIntervalForResource = 20
        let session = URLSession(configuration: configuration, delegate: self, delegateQueue: nil)
        defer { session.invalidateAndCancel() }
        var request = URLRequest(url: url)
        request.httpMethod = method
        request.httpBody = body
        if body != nil { request.setValue("application/json", forHTTPHeaderField: "Content-Type") }
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.setValue("mergegate-ios", forHTTPHeaderField: "X-Mergegate-Client")
        let (data, response) = try await session.data(for: request)
        guard let response = response as? HTTPURLResponse else { throw URLError(.badServerResponse) }
        guard (200..<300).contains(response.statusCode) else {
            let envelope = try? JSONDecoder().decode(FailureEnvelope.self, from: data)
            throw HTTPFailure(
                status: response.statusCode, code: envelope?.error.code ?? "http_error",
                message: envelope?.error.message ?? "The hub returned HTTP \(response.statusCode).",
                hint: envelope?.error.hint
            )
        }
        return data
    }

    // A saved hub must never silently redirect a request to a different origin.
    public func urlSession(
        _ session: URLSession, task: URLSessionTask,
        willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest
    ) async -> URLRequest? { nil }
}

private struct FailureEnvelope: Decodable {
    struct Body: Decodable {
        let code: String
        let message: String
        let hint: String?
    }
    let error: Body
}
