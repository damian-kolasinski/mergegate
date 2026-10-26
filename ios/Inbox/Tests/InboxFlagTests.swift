import Foundation
import Testing

@testable import Inbox

struct InboxFlagTests {
    @Test
    func flagsUseTheWebRowsWordsAndOrder() throws {
        let item = try decode(
            #"""
            {"id":"app#1","name":"app#42","title":"T","state":"changes_requested","revision":2,
             "questions":1,"rejections":2,"votes":3,"target":"main","parent":"app#41","behind":true,
             "demo":true,"changed_since":{"revision":1,"files":3,"approved":true},
             "blocked":{"kind":"conflict","paths":["a.swift"]},
             "mergeable":{"kind":"conflict","target_commit":"abc","paths":["a.swift"]},
             "files":6,"review_files":4,"additions":10,"deletions":2}
            """#)
        #expect(item.flags.map(\.label) == [
            "1 question", "2 rejections", "3 to vote on",
            "changes requested · waiting for r3", "3 changed since approved r1",
            "↳ behind app#41", "Conflicts with main", "▶ demo",
        ])
        #expect(item.flags.filter(\.opensDecisions).count == 3)
        #expect(item.filesLabel == "4 of 6 files")
    }

    @Test
    func preApprovalConflictShowsOnceAndCleanShowsNothing() throws {
        let conflicting = try decode(
            #"{"id":"a#1","name":"a#1","title":"T","state":"needs_review","revision":1,"target":"main","mergeable":{"kind":"conflict","target_commit":"c","paths":["x"]},"files":1,"review_files":1,"additions":1,"deletions":0}"#
        )
        #expect(conflicting.flags.map(\.label) == ["Conflicts with main"])
        #expect(conflicting.mergeable == .conflict(paths: ["x"]))
        #expect(conflicting.filesLabel == "1 file")
        let clean = try decode(
            #"{"id":"a#2","name":"a#2","title":"T","state":"needs_review","revision":1,"mergeable":{"kind":"clean"},"review_files":1,"additions":1,"deletions":0}"#
        )
        #expect(clean.flags.isEmpty)
        #expect(clean.filesLabel == nil)
        let approvedUnchanged = try decode(
            #"{"id":"a#3","name":"a#3","title":"T","state":"needs_review","revision":2,"changed_since":{"revision":1,"files":0,"approved":true},"blocked":{"kind":"waiting","parent":"a#2"},"review_files":1,"additions":1,"deletions":0}"#
        )
        #expect(approvedUnchanged.flags.map(\.label) == ["changed since approved r1", "Approved"])
    }

    @Test
    func ageMatchesTheWebColumn() throws {
        let item = try decode(
            #"{"id":"a#1","name":"a#1","title":"T","state":"needs_review","revision":1,"updated_at":"2026-10-26T09:00:00.123Z","review_files":1,"additions":0,"deletions":0}"#
        )
        let updated = try Date("2026-10-26T09:00:00.123Z", strategy: .iso8601.year().month().day().time(includingFractionalSeconds: true))
        #expect(item.age(now: updated.addingTimeInterval(30)) == "now")
        #expect(item.age(now: updated.addingTimeInterval(5 * 60)) == "5m")
        #expect(item.age(now: updated.addingTimeInterval(2 * 3600)) == "2h")
        #expect(item.age(now: updated.addingTimeInterval(3 * 86400)) == "3d")
    }

    private func decode(_ json: String) throws -> InboxItem {
        try JSONDecoder().decode(InboxItem.self, from: Data(json.utf8))
    }
}
