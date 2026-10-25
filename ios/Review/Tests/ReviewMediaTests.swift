import Foundation
import Networking
import Testing
import UIKit

@testable import Review

struct ReviewMediaTests {
    @Test func mutableZIPIsFetchedAgainWhileContentAddressedMediaIsCached() async throws {
        let directory = FileManager.default.temporaryDirectory.appending(
            component: UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let transport = MediaTransport()
        let service = ReviewSourceService(
            transport: transport, url: URL(string: "http://hub")!,
            cache: ReviewCache(directory: directory))
        let first = try await service.media(path: "/api/items/sample%231/demo.zip")
        let second = try await service.media(path: "/api/items/sample%231/demo.zip")
        #expect(first != second)
        let immutable = try await service.media(path: "/api/attachments/abc")
        #expect(try await service.media(path: "/api/attachments/abc") == immutable)
        #expect(await transport.reads == 3)
    }
    @Test @MainActor
    func resizedImagesCompareOutsideTheSmallerImageAndIdenticalPixelsStayUnchanged()
        async throws
    {
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        let before = UIGraphicsImageRenderer(size: CGSize(width: 1, height: 1), format: format)
            .image {
                context in
                UIColor.red.setFill()
                context.fill(CGRect(x: 0, y: 0, width: 1, height: 1))
            }
        let after = UIGraphicsImageRenderer(size: CGSize(width: 2, height: 1), format: format).image
        {
            context in
            UIColor.red.setFill()
            context.fill(CGRect(x: 0, y: 0, width: 2, height: 1))
        }
        let difference = try #require(await ReviewImages.shared.compare(before, after))
        #expect(difference.changed == 1 && difference.total == 2)
        let unchanged = try #require(await ReviewImages.shared.compare(before, before))
        #expect(unchanged.changed == 0 && unchanged.total == 1)
        let decoded = try await ReviewImages.shared.decode(#require(before.pngData()))
        #expect(decoded.size == before.size)
        await #expect(throws: (any Error).self) {
            try await ReviewImages.shared.decode(Data("invalid image".utf8))
        }
    }
}
private actor MediaTransport: HTTPTransport {
    var reads = 0
    func get(_ url: URL) async throws -> Data {
        reads += 1
        return Data("response-\(reads)".utf8)
    }
    func post(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }
    func put(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }
    func delete(_ url: URL) async throws -> Data { throw URLError(.unsupportedURL) }
}
