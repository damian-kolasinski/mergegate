import Foundation

// One badge of the web row's flags column (ui/inbox/inbox.tsx), in the same order and words.
struct InboxFlag: Equatable, Identifiable {
    enum Tone: Equatable { case brand, warning, neutral, plain }
    let label: String
    let tone: Tone
    var solid = false
    /// Questions, rejections and votes wait on the operator; tapping them opens the decisions.
    var opensDecisions = false
    var id: String { label }
}

extension InboxItem {
    var flags: [InboxFlag] {
        var flags: [InboxFlag] = []
        if let count = questions, count > 0 {
            flags.append(
                .init(
                    label: "\(count) \(count == 1 ? "question" : "questions")", tone: .brand,
                    solid: true, opensDecisions: true))
        }
        if let count = rejections, count > 0 {
            flags.append(
                .init(
                    label: "\(count) \(count == 1 ? "rejection" : "rejections")", tone: .warning,
                    solid: true, opensDecisions: true))
        }
        if let count = votes, count > 0 {
            flags.append(
                .init(label: "\(count) to vote on", tone: .warning, solid: true, opensDecisions: true))
        }
        if state == "changes_requested" {
            flags.append(.init(label: "changes requested · waiting for r\(revision + 1)", tone: .plain))
        }
        if let changed = changedSince {
            // An approval that stands with nothing changed reads "changed since approved r2".
            let count = changed.approved && changed.files == 0 ? "" : "\(changed.files) "
            flags.append(
                .init(
                    label:
                        "\(count)changed since \(changed.approved ? "approved " : "")r\(changed.revision)",
                    tone: .plain))
        }
        if let parent {
            flags.append(
                .init(
                    label: "↳ \(behind == true ? "behind" : "on") \(parent)",
                    tone: behind == true ? .warning : .neutral))
        }
        if let blocked {
            flags.append(
                .init(
                    label: blocked.label(target: target),
                    tone: blocked.kind == "waiting" ? .brand : .warning, solid: true))
        } else if case .conflict = mergeable {
            flags.append(
                .init(label: "Conflicts with \(target ?? "target")", tone: .warning, solid: true))
        }
        if demo == true { flags.append(.init(label: "▶ demo", tone: .brand)) }
        return flags
    }

    /// `4 of 6 files` when rules trusted some, so the total sets them apart.
    var filesLabel: String? {
        guard let files else { return nil }
        let all = "\(files) \(files == 1 ? "file" : "files")"
        return reviewFiles == files ? all : "\(reviewFiles) of \(all)"
    }

    /// How long ago the item changed, as short as the web column: `now`, `5m`, `2h`, `3d`.
    func age(now: Date) -> String? {
        guard let updatedAt,
            let date = (try? Date(
                updatedAt, strategy: Date.ISO8601FormatStyle(includingFractionalSeconds: true)))
                ?? (try? Date(updatedAt, strategy: .iso8601))
        else { return nil }
        let minutes = Int(now.timeIntervalSince(date) / 60)
        if minutes < 1 { return "now" }
        if minutes < 60 { return "\(minutes)m" }
        let hours = minutes / 60
        return hours < 24 ? "\(hours)h" : "\(hours / 24)d"
    }
}

extension InboxItem.Blocked {
    func label(target: String?) -> String {
        switch kind {
        case "held": "Merge held"
        case "conflict": "Conflicts with \(target ?? "target")"
        case "waiting": "Approved"
        case "behind": "Behind"
        default: kind
        }
    }
}
