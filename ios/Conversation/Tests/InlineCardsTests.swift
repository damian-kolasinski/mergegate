import ConversationInterface
import Foundation
import Testing

@testable import Conversation

struct InlineCardsTests {
    @Test func threadsNeedingTheOperatorAndWhereTheirCardsGo() throws {
        let snapshot = try JSONDecoder().decode(
            ConversationSnapshot.self,
            from: Data(
                """
                {"comments":[
                  {"id":"q","revision":2,"path":"a.swift","side":"new","start_line":3,"line":5,"body":"Why?","quote":null,"placement":null,"external":null},
                  {"id":"vote","revision":2,"path":"a.swift","side":"old","start_line":7,"line":7,"body":"Nit","quote":null,"placement":null,
                   "external":{"author":"lint-bot","path":"a.swift","pending":true,"summary":false,"vote":null,"lapsed":false,"agreed_reply":null}},
                  {"id":"done","revision":1,"path":"a.swift","side":"new","start_line":1,"line":1,"body":"Old","quote":null,"placement":{"kind":"outdated","start_line":null,"line":null},"external":null},
                  {"id":"rejected","revision":2,"path":"a.swift","side":"new","start_line":9,"line":9,"body":"Cache it","quote":null,"placement":null,
                   "external":{"author":"reviewer","path":"a.swift","pending":false,"summary":false,"vote":"up","lapsed":false,"agreed_reply":null}}],
                 "replies":[
                  {"id":1,"comment":"q","author":"agent","status":"question","body":"Which client?","created_at":"2026-10-25T10:00:00Z"},
                  {"id":2,"comment":"rejected","author":"agent","status":"rejected","body":"Stays fresh.","created_at":"2026-10-25T10:01:00Z"}]}
                """.utf8))
        let threads = snapshot.threads(revision: 2)
        #expect(threads.filter(\.needsYou).map(\.id) == ["q", "vote", "rejected"])
        // A card sits under the last line of its range, on its side; an outdated thread has none.
        #expect(threads.map(\.mark) == [
            SourceMark(side: .new, line: 5), SourceMark(side: .old, line: 7), nil,
            SourceMark(side: .new, line: 9),
        ])
        let answered = ConversationThread(
            comment: threads[0].comment,
            replies: threads[0].replies + [
                try JSONDecoder().decode(
                    ThreadReply.self,
                    from: Data(
                        #"{"id":3,"comment":"q","author":"operator","status":null,"body":"Both.","created_at":"2026-10-25T10:02:00Z"}"#
                            .utf8))
            ], revision: 2)
        #expect(!answered.needsYou)
    }
}
