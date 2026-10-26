import Foundation

// The order the file pill steps through and the file tree lists: files to review as the web's
// tree lays them out, then those unchanged since the compared revision, then trusted ones.
struct ReviewReadingOrder {
    let review: [ReviewFile]
    let unchanged: [ReviewFile]
    let trusted: [ReviewFile]
    var files: [ReviewFile] { review + unchanged + trusted }

    init(_ snapshot: ReviewSnapshot) {
        let unchanged = { (file: ReviewFile) in snapshot.since != nil && file.interdiff == .unchanged }
        review = ReviewFileTree.order(
            snapshot.files.filter { $0.tier == .review && !unchanged($0) })
        self.unchanged = snapshot.files.filter(unchanged)
        trusted = ReviewFileTree.order(
            snapshot.files.filter { $0.tier == .trusted && !unchanged($0) })
    }

    func position(of path: String?) -> Int? { files.firstIndex { $0.path == path } }

    func next(after path: String?) -> String? {
        guard let index = position(of: path), index + 1 < files.count else { return nil }
        return files[index + 1].path
    }

    func previous(before path: String?) -> String? {
        guard let index = position(of: path), index > 0 else { return nil }
        return files[index - 1].path
    }

    /// The next file still to view after `path`, wrapping to earlier ones.
    func nextUnviewed(after path: String?) -> String? {
        let start = (position(of: path) ?? -1) + 1
        let wrapped = files[min(start, files.count)...] + files[..<min(start, files.count)]
        return wrapped.first { $0.needsViewing && $0.path != path }?.path
    }
}

extension ReviewFile {
    /// A review-tier file not yet marked viewed; trusted and dropped files never need it.
    var needsViewing: Bool { tier == .review && !viewed && interdiff != .dropped }
}

// What the review bar's main button does: finish viewing files, then send waiting drafts, then
// approve. Its menu always offers every outcome, so the label changing never hides one.
enum ReviewPrimaryAction: Equatable {
    case viewedAndNext
    case nextUnviewed(String)
    case send(Int)
    case approve

    /// `reviewable` is the item's newest state; the displayed snapshot may be older.
    init?(
        snapshot: ReviewSnapshot, reviewable: Bool, order: ReviewReadingOrder, selected: String?,
        drafts: Int
    ) {
        guard reviewable else { return nil }
        let current = snapshot.files.first { $0.path == selected }
        if snapshot.remaining > 0 {
            if current?.needsViewing == true {
                self = .viewedAndNext
                return
            }
            if let next = order.nextUnviewed(after: selected) {
                self = .nextUnviewed(next)
                return
            }
        }
        self = drafts > 0 ? .send(drafts) : .approve
    }

    var title: String {
        switch self {
        case .viewedAndNext: "Viewed & next"
        case .nextUnviewed: "Next to view"
        case .send(let count): "Send \(count)"
        case .approve: "Approve & merge"
        }
    }
}
