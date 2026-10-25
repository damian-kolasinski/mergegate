import Foundation
import SwiftData

@Model
final class StoredFeedbackRecord {
    @Attribute(.unique) var scope: String
    var payload: Data

    init(scope: String, payload: Data) {
        self.scope = scope
        self.payload = payload
    }
}
