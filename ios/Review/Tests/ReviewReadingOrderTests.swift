import Foundation
import Testing

@testable import Review

struct ReviewReadingOrderTests {
    @Test
    func treeGroupsFoldersAndNestsTestsLikeTheWeb() throws {
        // Hub order: the most important file first, its test later, a lone folder file last.
        let snapshot = try decode(
            files: [
                file("app/Login/LoginView.swift", importance: 82, additions: 10),
                file("app/Login/LoginModel.swift", importance: 40, additions: 3),
                file("docs/guide/setup.md"),
                file("app/Login/LoginViewTests.swift", additions: 5),
            ])
        let tree = ReviewFileTree.build(snapshot.files, prefix: "r:")
        #expect(tree.map(\.id) == ["r:app/Login/", "r:docs/guide/"])
        guard case .group(let login) = tree[0], case .group(let docs) = tree[1] else {
            Issue.record("expected folders")
            return
        }
        // A folder holding only another says both on one row.
        #expect(login.name == "app/Login")
        #expect(docs.name == "docs/guide")
        #expect(login.peak == 82)
        #expect(login.additions == 18)
        guard case .file(let view) = login.nodes[0] else {
            Issue.record("expected a file")
            return
        }
        #expect(view.file.path == "app/Login/LoginView.swift")
        #expect(view.tests.map(\.path) == ["app/Login/LoginViewTests.swift"])
        #expect(ReviewFileTree.order(snapshot.files).map(\.path) == [
            "app/Login/LoginView.swift", "app/Login/LoginViewTests.swift",
            "app/Login/LoginModel.swift", "docs/guide/setup.md",
        ])
        #expect(ReviewFileTree.tested("login_tests.rs") == "login")
        #expect(ReviewFileTree.tested("test_login.py") == "login")
        #expect(ReviewFileTree.tested("button.spec.ts") == "button")
        #expect(ReviewFileTree.tested("Login.swift") == nil)
    }

    @Test
    func modulesNameTheirFolderOnceAndPairTestsAcrossTargets() throws {
        let snapshot = try decode(
            files: [
                file("Products/Chat/Sources/Thread.swift", module: ("Products/Chat", nil, "Thread.swift")),
                file(
                    "Products/Chat/Tests/Thread.swift",
                    module: ("Products/Chat", "Tests", "Thread.swift")),
            ])
        let tree = ReviewFileTree.build(snapshot.files, prefix: "")
        guard case .group(let chat) = tree.first else {
            Issue.record("expected a module")
            return
        }
        #expect(chat.kind == .module)
        #expect(chat.name == "Chat")
        #expect(chat.sub == "Products")
        guard case .file(let thread) = chat.nodes.first else {
            Issue.record("expected the code file")
            return
        }
        #expect(thread.tests.map(\.path) == ["Products/Chat/Tests/Thread.swift"])
    }

    @Test
    func orderSkipsTrustedAndUnchangedToTheEndAndFindsTheNextFileToView() throws {
        let snapshot = try decode(
            since: 1,
            files: [
                file("a.swift", viewed: true, interdiff: "changed"),
                file("b.swift", interdiff: "unchanged"),
                file("c.swift", interdiff: "added"),
                file("Package.resolved", tier: "trusted"),
                file("d.swift", interdiff: "changed"),
            ])
        let order = ReviewReadingOrder(snapshot)
        #expect(order.files.map(\.path) == ["a.swift", "c.swift", "d.swift", "b.swift", "Package.resolved"])
        #expect(order.next(after: "a.swift") == "c.swift")
        #expect(order.previous(before: "a.swift") == nil)
        #expect(order.next(after: "Package.resolved") == nil)
        #expect(order.nextUnviewed(after: "c.swift") == "d.swift")
        // An unchanged file still unviewed needs reading too.
        #expect(order.nextUnviewed(after: "d.swift") == "b.swift")
        // Wraps to earlier files and never offers the trusted one.
        #expect(order.nextUnviewed(after: "b.swift") == "c.swift")
    }

    @Test
    func primaryActionViewsFilesThenSendsDraftsThenApproves() throws {
        let reading = try decode(files: [file("a.swift"), file("b.swift", viewed: true)])
        let order = ReviewReadingOrder(reading)
        #expect(
            ReviewPrimaryAction(
                snapshot: reading, reviewable: true, order: order, selected: "a.swift", drafts: 2)
                == .viewedAndNext)
        #expect(
            ReviewPrimaryAction(
                snapshot: reading, reviewable: true, order: order, selected: "b.swift", drafts: 0)
                == .nextUnviewed("a.swift"))
        let done = try decode(files: [file("a.swift", viewed: true)])
        let doneOrder = ReviewReadingOrder(done)
        #expect(
            ReviewPrimaryAction(
                snapshot: done, reviewable: true, order: doneOrder, selected: "a.swift", drafts: 3)
                == .send(3))
        #expect(
            ReviewPrimaryAction(
                snapshot: done, reviewable: true, order: doneOrder, selected: "a.swift", drafts: 0)
                == .approve)
        #expect(
            ReviewPrimaryAction(
                snapshot: done, reviewable: false, order: doneOrder, selected: "a.swift", drafts: 0)
                == nil)
    }

    private func file(
        _ path: String, tier: String = "review", viewed: Bool = false, interdiff: String? = nil,
        importance: Int? = nil, additions: Int? = nil,
        module: (String, String?, String)? = nil
    ) -> String {
        var fields = [
            "\"path\":\"\(path)\"", "\"tier\":\"\(tier)\"", "\"viewed\":\(viewed)",
            "\"content\":{\"kind\":\"text\",\"hunks\":[]}",
        ]
        if let interdiff { fields.append("\"interdiff\":\"\(interdiff)\"") }
        if let importance { fields.append("\"importance\":\(importance)") }
        if let additions { fields.append("\"additions\":\(additions),\"deletions\":0") }
        if let (path, layer, inner) = module {
            fields.append(
                "\"module\":{\"path\":\"\(path)\",\"layer\":\(layer.map { "\"\($0)\"" } ?? "null"),\"inner\":\"\(inner)\"}"
            )
        }
        return "{\(fields.joined(separator: ","))}"
    }

    private func decode(since: Int? = nil, files: [String]) throws -> ReviewSnapshot {
        try JSONDecoder().decode(
            ReviewSnapshot.self,
            from: Data(
                """
                {"item":{"id":"app#1","name":"app#1","title":"T","target":"main","state":"needs_review","revision":{"number":2}},
                 "since":\(since.map(String.init) ?? "null"),"interdiffs":[1],"files":[\(files.joined(separator: ","))]}
                """.utf8))
    }
}
