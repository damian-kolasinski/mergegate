import ConversationInterface
import DiffDocumentInterface
import Foundation
import Testing

@testable import Review

struct ReviewAnnotatedLinesTests {
    private let hunk = DiffHunk(
        header: "@@ -1,3 +1,3 @@",
        lines: [
            .init(kind: .context, oldLine: 1, newLine: 1, text: "a", hasNewline: true),
            .init(kind: .removed, oldLine: 2, newLine: nil, text: "b", hasNewline: true),
            .init(kind: .added, oldLine: nil, newLine: 2, text: "B", hasNewline: true),
            .init(kind: .context, oldLine: 3, newLine: 3, text: "c", hasNewline: true),
        ])

    @Test
    func cardsFollowTheLineTheyEndOnWithTheHeaderOnlyFirst() {
        let segments = ReviewAnnotatedLines.split(
            hunk,
            marks: [.init(side: .old, line: 2), .init(side: .new, line: 2), .init(side: .new, line: 9)])
        #expect(segments.map { $0.hunk.lines.map(\.text) } == [["a", "b"], ["B"], ["c"]])
        #expect(segments.map(\.marks) == [[.init(side: .old, line: 2)], [.init(side: .new, line: 2)], []])
        #expect(segments.map(\.hunk.header) == ["@@ -1,3 +1,3 @@", "", ""])
    }

    @Test
    func unchangedContextTakesAThreadOnEitherSideAndNoMarksKeepOneBlock() {
        let both = ReviewAnnotatedLines.split(
            hunk, marks: [.init(side: .old, line: 1), .init(side: .new, line: 3)])
        #expect(both.map { $0.hunk.lines.map(\.text) } == [["a"], ["b", "B", "c"]])
        #expect(ReviewAnnotatedLines.split(hunk, marks: []).count == 1)
    }

    @Test
    func aNoNewlineNoteStaysWithItsLineAndOnlyTheBlockShowingTheStartReveals() {
        let ending = DiffHunk(
            header: "",
            lines: [
                .init(kind: .added, oldLine: nil, newLine: 1, text: "x", hasNewline: false),
                .init(kind: .note, oldLine: nil, newLine: nil, text: "\\ No newline at end of file"),
                .init(kind: .added, oldLine: nil, newLine: 2, text: "y", hasNewline: true),
            ])
        let segments = ReviewAnnotatedLines.split(ending, marks: [.init(side: .new, line: 1)])
        #expect(segments.map { $0.hunk.lines.count } == [2, 1])
        let split = ReviewAnnotatedLines.split(hunk, marks: [.init(side: .old, line: 2)])
        let anchor = SourceAnchor(side: .new, startLine: 2, endLine: 3, quote: "")
        #expect(split.map { ReviewAnnotatedLines.shows($0.hunk, start: anchor) } == [false, true])
    }

    @Test
    func conversationsCountWhatWaitsOnTheOperatorLikeTheInbox() throws {
        let snapshot = try JSONDecoder().decode(
            ReviewSnapshot.self,
            from: Data(
                """
                {"item":{"id":"app#1","name":"app#1","title":"T","target":"main","state":"needs_review","revision":{"number":2}},
                 "since":null,"interdiffs":[],"files":[],
                 "comments":[
                   {"id":"q","external":null},
                   {"id":"vote","external":{"pending":true,"vote":null,"agreed_reply":null}},
                   {"id":"voted","external":{"pending":true,"vote":"up","agreed_reply":null}},
                   {"id":"rejected","external":{"pending":false,"vote":null,"agreed_reply":null}},
                   {"id":"accepted","external":{"pending":false,"vote":null,"agreed_reply":7}},
                   {"id":"summary","external":{"pending":true,"vote":null,"agreed_reply":null,"summary":true}}],
                 "replies":[
                   {"id":1,"comment":"q","author":"agent","status":"question"},
                   {"id":6,"comment":"rejected","author":"agent","status":"rejected"},
                   {"id":7,"comment":"accepted","author":"agent","status":"rejected"}]}
                """.utf8))
        #expect(snapshot.needsYou == 3)
    }
}
