import Foundation

extension ReviewSourceService {
    func blob(project: String, id: String) async throws -> Data {
        try await media(
            path: url.appending(path: "api/projects").appending(component: project)
                .appending(path: "blobs").appending(component: id))
    }

    func media(path: String) async throws -> Data {
        let endpoint = try mediaURL(path)
        if endpoint.path.hasPrefix("/api/attachments/") { return try await media(path: endpoint) }
        return try await transport.get(endpoint)
    }

    func mediaURL(_ path: String) throws -> URL {
        // Attachment URLs are hub-relative. Never send trusted requests to a supplied origin.
        guard path.hasPrefix("/api/"), !path.hasPrefix("//"),
            let endpoint = URL(string: path, relativeTo: url)?.absoluteURL,
            endpoint.scheme == url.scheme, endpoint.host == url.host, endpoint.port == url.port,
            endpoint.user == nil, endpoint.password == nil
        else { throw URLError(.unsupportedURL) }
        return endpoint
    }

    private func media(path endpoint: URL) async throws -> Data {
        if let saved = await cache?.read(endpoint.absoluteString) { return saved.data }
        let data = try await transport.get(endpoint)
        await cache?.save(data, key: endpoint.absoluteString)
        return data
    }

    func export(_ data: Data, name: String) throws -> URL {
        let directory = FileManager.default.temporaryDirectory.appending(
            component: UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let file = directory.appending(component: URL(fileURLWithPath: name).lastPathComponent)
        try data.write(to: file, options: .atomic)
        return file
    }
}
