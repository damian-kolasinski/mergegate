import SwiftUI

struct ConversationThreadView: View {
    let thread: ConversationThread

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                VStack(alignment: .leading, spacing: 8) {
                    Text(thread.status).appearanceText(Appearance.heading)
                        .accessibilityIdentifier("threadStatus")
                    Text(thread.comment.filePath ?? "Whole item").appearanceText(Appearance.rowHeading)
                        .accessibilityLabel(thread.comment.filePath.map { "File: \($0)" } ?? "Whole item")
                    Text("\(thread.location) · Written on r\(thread.comment.revision)")
                        .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                        .accessibilityIdentifier("threadLocation")
                    if let placement = thread.placementLabel {
                        Text(placement).appearanceText(Appearance.caption)
                    }
                    if let quote = thread.comment.quote, !quote.isEmpty {
                        Text(quote).font(.system(.body, design: .monospaced))
                            .padding(12).frame(maxWidth: .infinity, alignment: .leading)
                            .background(Appearance.surface, in: RoundedRectangle(cornerRadius: 8))
                            .accessibilityIdentifier("threadQuote")
                    }
                }
                Divider()
                VStack(alignment: .leading, spacing: 8) {
                    Text(thread.comment.external?.author ?? "You")
                        .appearanceText(Appearance.rowHeading)
                    if let external = thread.comment.external {
                        Text("GitHub · \(external.status)").appearanceText(Appearance.caption)
                            .foregroundStyle(Appearance.secondary)
                    }
                    Text(thread.comment.body).appearanceText(Appearance.body)
                        .accessibilityIdentifier("threadComment")
                }
                ForEach(thread.replies) { reply in
                    Divider()
                    VStack(alignment: .leading, spacing: 8) {
                        Text(reply.author == "operator" ? "You" : "Agent")
                            .appearanceText(Appearance.rowHeading)
                        if let status = reply.statusLabel {
                            Text(status).appearanceText(Appearance.caption).foregroundStyle(Appearance.brand)
                        }
                        if reply.overrides == true {
                            Text("Override").appearanceText(Appearance.caption).foregroundStyle(
                                Appearance.brand)
                        }
                        Text("Sent \(reply.timestamp)").appearanceText(Appearance.caption)
                            .foregroundStyle(Appearance.secondary)
                        if !reply.body.isEmpty {
                            Text(reply.body).appearanceText(Appearance.body)
                        }
                    }.accessibilityIdentifier("threadReply-\(reply.id)")
                }
            }.padding(20)
        }.background(Appearance.page)
            .navigationTitle("Thread").navigationBarTitleDisplayMode(.inline)
            .accessibilityIdentifier("conversationThread")
    }
}
