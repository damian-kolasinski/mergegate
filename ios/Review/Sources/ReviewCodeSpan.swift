import DiffDocumentInterface
import Foundation

// Tuple shape and UTF-16 units are defined by crates/api/src/highlight.rs.
struct ReviewCodeSpan: Decodable, Sendable {
    let value: CodeSpan

    init(from decoder: any Decoder) throws {
        var tuple = try decoder.unkeyedContainer()
        value = CodeSpan(
            start: Int(try tuple.decode(UInt32.self)), length: Int(try tuple.decode(UInt32.self)),
            tag: try tuple.decode(String.self))
    }
}
