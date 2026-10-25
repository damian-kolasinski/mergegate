import Foundation

// MarkViewed in crates/api/src/viewed.rs always names the displayed revision.
struct ReviewViewedChange: Encodable, Sendable {
    let revision: UInt32
    let path: String
    let viewed: Bool
}

struct ReviewViewedFailure {
    let change: ReviewViewedChange
    let message: String
}
