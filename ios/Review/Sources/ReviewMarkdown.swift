import SwiftUI

struct ReviewMarkdown: View {
    let source: String
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            ForEach(Array(MarkdownBlock.parse(source).enumerated()), id: \.offset) { _, block in
                switch block {
                case .heading(let text):
                    Text(inline(text)).appearanceText(Appearance.heading).accessibilityAddTraits(
                        .isHeader)
                case .code(let text):
                    ScrollView(.horizontal) {
                        Text(text).font(.system(.body, design: .monospaced)).textSelection(.enabled)
                            .padding(12)
                    }.background(Appearance.surface, in: RoundedRectangle(cornerRadius: Appearance.controlRadius))
                case .text(let text):
                    Text(inline(text)).appearanceText(Appearance.body).textSelection(.enabled)
                case .rule: Divider()
                case .table(let rows):
                    ScrollView(.horizontal) {
                        Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 8) {
                            ForEach(Array(rows.enumerated()), id: \.offset) { index, row in
                                GridRow {
                                    ForEach(Array(row.enumerated()), id: \.offset) { _, cell in
                                        Text(inline(cell)).bold(index == 0).textSelection(.enabled)
                                    }
                                }
                            }
                        }.appearanceText(Appearance.body).padding(10)
                    }.background(Appearance.surface)
                }
            }
        }.frame(maxWidth: .infinity, alignment: .leading)
    }
    private func inline(_ source: String) -> AttributedString {
        var result =
            (try? AttributedString(
                markdown: source, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)
            ))
            ?? AttributedString(source)
        for run in result.runs {
            if let url = run.link, !["https", "http"].contains(url.scheme?.lowercased() ?? "") {
                result[run.range].link = nil
            }
        }
        return result
    }
}

enum MarkdownBlock: Equatable {
    case heading(String)
    case text(String)
    case code(String)
    case table([[String]])
    case rule
    static func parse(_ source: String) -> [MarkdownBlock] {
        let lines = source.replacingOccurrences(of: "\r\n", with: "\n").components(
            separatedBy: "\n")
        var result: [MarkdownBlock] = []
        var index = 0
        while index < lines.count {
            let line = lines[index]
            let trim = line.trimmingCharacters(in: .whitespaces)
            index += 1
            if trim.isEmpty { continue }
            if trim.hasPrefix("```") || trim.hasPrefix("~~~") {
                let marker = String(trim.prefix(3))
                var body: [String] = []
                while index < lines.count
                    && !lines[index].trimmingCharacters(in: .whitespaces).hasPrefix(marker)
                {
                    body.append(lines[index])
                    index += 1
                }
                if index < lines.count { index += 1 }
                result.append(.code(body.joined(separator: "\n")))
            } else if trim.hasPrefix("#"), let space = trim.firstIndex(of: " "),
                trim[..<space].allSatisfy({ $0 == "#" })
            {
                result.append(.heading(String(trim[trim.index(after: space)...])))
            } else if ["---", "***", "___"].contains(trim) {
                result.append(.rule)
            } else if line.contains("|"), index < lines.count,
                lines[index].contains("-"), lines[index].allSatisfy({ " |:-\t".contains($0) })
            {
                var rows = [cells(line)]
                index += 1
                while index < lines.count && lines[index].contains("|") {
                    rows.append(cells(lines[index]))
                    index += 1
                }
                result.append(.table(rows))
            } else {
                result.append(.text(line))
            }
        }
        return result
    }
    private static func cells(_ line: String) -> [String] {
        var parts = line.components(separatedBy: "|").map {
            $0.trimmingCharacters(in: .whitespaces)
        }
        if parts.first == "" { parts.removeFirst() }
        if parts.last == "" { parts.removeLast() }
        return parts
    }
}
