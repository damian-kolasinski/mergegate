import Foundation
import Network

struct HubAddress {
    let url: URL

    init(_ text: String) throws {
        let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard var parts = URLComponents(string: text),
            ["http", "https"].contains(parts.scheme?.lowercased() ?? ""),
            let host = parts.host, !host.isEmpty,
            parts.user == nil, parts.password == nil,
            parts.query == nil,
            // `#/` is the web inbox, as `mg hub status` prints the review UI's link.
            parts.fragment == nil || parts.fragment == "/",
            parts.path.isEmpty || parts.path == "/",
            parts.port.map({ (1...65535).contains($0) }) ?? true,
            Self.isTailnetHost(host)
        else {
            throw AddressError()
        }
        if parts.fragment != nil {
            // Stored as the bare address users typed before, which keys the caches.
            parts.fragment = nil
            parts.path = ""
        }
        guard let url = parts.url else { throw AddressError() }
        self.url = url
    }

    private static func isTailnetHost(_ host: String) -> Bool {
        let host = host.lowercased()
        let components = host.split(separator: ".")
        let octets = components.compactMap { UInt8($0) }
        if components.count == 4 && octets.count == 4 {
            return octets[0] == 100 && (64...127).contains(octets[1])
        }
        if host.contains(":") {
            let literal = host.trimmingCharacters(in: CharacterSet(charactersIn: "[]"))
            guard let address = IPv6Address(literal) else { return false }
            return address.rawValue.prefix(6) == Data([0xfd, 0x7a, 0x11, 0x5c, 0xa1, 0xe0])
        }
        let labels = host.split(separator: ".", omittingEmptySubsequences: false)
        let valid = labels.allSatisfy { label in
            !label.isEmpty && label.count <= 63 && label.first != "-" && label.last != "-"
                && label.utf8.allSatisfy { (97...122).contains($0) || (48...57).contains($0) || $0 == 45 }
        }
        return valid && (labels.count == 1 || (host.hasSuffix(".ts.net") && labels.count >= 4))
    }
}

private struct AddressError: LocalizedError {
    var errorDescription: String? {
        "Enter the review UI link shown by mg hub status, such as http://my-mac:7800/#/."
    }
}
