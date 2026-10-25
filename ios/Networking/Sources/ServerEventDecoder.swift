import Foundation

struct ServerEventDecoder {
    private var line: [UInt8] = []
    private var data: [String] = []
    private var bytes = 0
    private var oversized = false
    private var afterCR = false
    private let limit = 256 * 1024

    mutating func append(_ byte: UInt8) -> String? {
        // AsyncBytes.lines drops empty lines, which are SSE frame delimiters.
        if byte == 10 && afterCR {
            afterCR = false
            return nil
        }
        afterCR = byte == 13
        if byte == 10 || byte == 13 { return finishLine() }
        if line.count < limit { line.append(byte) } else { oversized = true }
        return nil
    }

    private mutating func finishLine() -> String? {
        defer { line.removeAll(keepingCapacity: true) }
        if line.isEmpty {
            defer {
                data = []
                bytes = 0
                oversized = false
            }
            return data.isEmpty || oversized ? nil : data.joined(separator: "\n")
        }
        guard line.starts(with: Array("data:".utf8)) else { return nil }
        var value = line.dropFirst(5)
        if value.first == 32 { value = value.dropFirst() }
        bytes += value.count + 1
        if bytes > limit { oversized = true }
        if !oversized { data.append(String(decoding: value, as: UTF8.self)) }
        return nil
    }
}
