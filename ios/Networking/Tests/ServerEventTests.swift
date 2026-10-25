import Foundation
import Testing

@testable import Networking

struct ServerEventTests {
    @Test func rawByteStreamPreservesEmptyDelimitersUnicodeAndMultilineData() async {
        let bytes = Array(
            ": keep-alive\r\n\r\nevent: message\ndata: {\"type\":\ndata: \"highlights.changed\"}\n\ndata:é 🐙\r\r".utf8)
        let stream = AsyncStream<UInt8> { continuation in
            for byte in bytes { continuation.yield(byte) }
            continuation.finish()
        }
        var decoder = ServerEventDecoder()
        var received: [String] = []
        for await byte in stream {
            if let data = decoder.append(byte) { received.append(data) }
        }
        #expect(received == ["{\"type\":\n\"highlights.changed\"}", "é 🐙"])
    }

    @Test func oversizedFrameIsDroppedEntirelyAndTheNextFrameSurvives() {
        var decoder = ServerEventDecoder()
        let input = "data: partial\ndata: " + String(repeating: "x", count: 300_000) + "\n\ndata: next\n\n"
        let received = input.utf8.compactMap { decoder.append($0) }
        #expect(received == ["next"])
    }
}
