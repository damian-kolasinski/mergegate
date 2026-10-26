import Foundation

// The web's file tree (ui/item/tree.ts): each module the hub found, then folders in it, then
// files, a test under the file it covers. Files in no module sit in their own folders. Siblings
// come in review order, a group where its first file would, so the tree still leads with what
// to read first. Keep the two in step so both clients list and step through files alike.
enum ReviewTreeNode: Identifiable {
    case group(ReviewTreeGroup)
    case file(ReviewTreeLeaf)

    var id: String {
        switch self {
        case .group(let group): group.key
        case .file(let leaf): leaf.file.path
        }
    }
    var rank: Int {
        switch self {
        case .group(let group): group.rank
        case .file(let leaf): leaf.rank
        }
    }
    /// Every file below, as the tree lists them top to bottom.
    var files: [ReviewFile] {
        switch self {
        case .group(let group): group.nodes.flatMap(\.files)
        case .file(let leaf): [leaf.file] + leaf.tests
        }
    }
}

struct ReviewTreeGroup {
    enum Kind { case module, folder }
    let kind: Kind
    /// Unique in the tree, so a folded group stays folded across refreshes.
    let key: String
    /// A module's folder name, or a folder's with the ones it alone holds, as `Scenes/Chat`.
    var name: String
    /// Where a module lives and the folder all its files share, as `Products/App · Scenes/`.
    let sub: String
    let nodes: [ReviewTreeNode]
    /// The highest judge score in it; nil when none was judged.
    let peak: UInt8?
    let additions: UInt64
    let deletions: UInt64
    let rank: Int
}

struct ReviewTreeLeaf {
    let file: ReviewFile
    /// The tests of `file` beside it, which follow it.
    var tests: [ReviewFile]
    /// What else than plain code the file is, as `Tests` or `Interface`.
    let tag: String?
    var rank: Int
}

enum ReviewFileTree {
    static func build(_ files: [ReviewFile], prefix: String) -> [ReviewTreeNode] {
        let rank = Dictionary(
            files.enumerated().map { ($1.path, $0) }, uniquingKeysWith: { first, _ in first })
        var modules: [(String, [ReviewFile])] = []
        var loose: [Placed] = []
        for file in files {
            guard let module = file.module else {
                loose.append(.init(parts: file.path.split(separator: "/").map(String.init), file: file))
                continue
            }
            if let index = modules.firstIndex(where: { $0.0 == module.path }) {
                modules[index].1.append(file)
            } else {
                modules.append((module.path, [file]))
            }
        }
        var nodes = modules.map { self.module(prefix, $0.0, $0.1, rank) }
        nodes += folders(prefix, loose, rank)
        return ordered(nodes)
    }

    /// `files` as the tree lists them top to bottom, a test after the file it covers.
    static func order(_ files: [ReviewFile]) -> [ReviewFile] {
        build(files, prefix: "").flatMap(\.files)
    }

    private struct Placed {
        var parts: [String]
        let file: ReviewFile
    }

    private static func module(
        _ prefix: String, _ path: String, _ members: [ReviewFile], _ rank: [String: Int]
    ) -> ReviewTreeNode {
        var placed = members.map {
            Placed(
                parts: ($0.module?.inner ?? $0.path).split(separator: "/").map(String.init),
                file: $0)
        }
        // Said once on the module rather than on a folder row of its own.
        let shared = common(placed.map { Array($0.parts.dropLast()) })
        for index in placed.indices { placed[index].parts.removeFirst(shared.count) }
        let slash = path.lastIndex(of: "/")
        let sub = [
            slash.map { String(path[..<$0]) } ?? "",
            shared.isEmpty ? "" : shared.joined(separator: "/") + "/",
        ].filter { !$0.isEmpty }
        return group(
            .module, "\(prefix)m:\(path)",
            slash.map { String(path[path.index(after: $0)...]) } ?? path,
            sub.joined(separator: " · "), folders("\(prefix)m:\(path)/", placed, rank))
    }

    /// `placed` laid out by the folders left in their `parts`.
    private static func folders(_ key: String, _ placed: [Placed], _ rank: [String: Int])
        -> [ReviewTreeNode]
    {
        var here: [ReviewFile] = []
        var below: [(String, [Placed])] = []
        for each in placed {
            guard each.parts.count > 1, let first = each.parts.first else {
                here.append(each.file)
                continue
            }
            let inside = Placed(parts: Array(each.parts.dropFirst()), file: each.file)
            if let index = below.firstIndex(where: { $0.0 == first }) {
                below[index].1.append(inside)
            } else {
                below.append((first, [inside]))
            }
        }
        var nodes = paired(here, rank).map(ReviewTreeNode.file)
        for (name, inside) in below {
            var folder = group(.folder, "\(key)\(name)/", name, "", folders("\(key)\(name)/", inside, rank))
            // A folder holding only another says both on one row.
            if case .group(let outer) = folder, outer.nodes.count == 1,
                case .group(var only) = outer.nodes[0], only.kind == .folder
            {
                only.name = "\(name)/\(only.name)"
                folder = .group(only)
            }
            nodes.append(folder)
        }
        return ordered(nodes)
    }

    private static func group(
        _ kind: ReviewTreeGroup.Kind, _ key: String, _ name: String, _ sub: String,
        _ nodes: [ReviewTreeNode]
    ) -> ReviewTreeNode {
        let files = nodes.flatMap(\.files)
        return .group(
            .init(
                kind: kind, key: key, name: name, sub: sub, nodes: nodes,
                peak: files.compactMap(\.importance).max(),
                additions: files.reduce(0) { $0 + ($1.additions ?? 0) },
                deletions: files.reduce(0) { $0 + ($1.deletions ?? 0) },
                rank: nodes.map(\.rank).min() ?? 0))
    }

    /// One folder's files, each test after the file it covers when that's here too.
    private static func paired(_ files: [ReviewFile], _ rank: [String: Int]) -> [ReviewTreeLeaf] {
        var subjects: [String: [ReviewFile]] = [:]
        for file in files where !isTest(file) {
            subjects[stem(name(file)), default: []].append(file)
        }
        var tests: [String: [ReviewFile]] = [:]
        var leaves: [ReviewTreeLeaf] = []
        for file in files {
            // A file in a tests target covers its namesake, as `Login.swift` there covers
            // `Login.swift` in the code's.
            if isTest(file), let subject = subject(
                of: file, among: subjects[tested(name(file)) ?? stem(name(file))] ?? [])
            {
                tests[subject.path, default: []].append(file)
                continue
            }
            leaves.append(
                .init(
                    file: file, tests: [],
                    tag: file.module?.layer ?? (isTest(file) ? "Tests" : nil),
                    rank: rank[file.path] ?? 0))
        }
        for index in leaves.indices {
            let covering = tests[leaves[index].file.path] ?? []
            leaves[index].tests = covering
            leaves[index].rank = ([leaves[index].rank] + covering.map { rank[$0.path] ?? 0 }).min()!
        }
        return leaves
    }

    /// Which namesake `test` covers: the one of its own kind, as `button.ts` rather than
    /// `button.css` for `button.test.ts`, else the first by path, never by arrival order.
    private static func subject(of test: ReviewFile, among namesakes: [ReviewFile]) -> ReviewFile? {
        let kind = fileExtension(name(test))
        return namesakes.sorted {
            let a = fileExtension(name($0)) == kind
            let b = fileExtension(name($1)) == kind
            return a != b ? a : $0.path < $1.path
        }.first
    }

    private static func ordered(_ nodes: [ReviewTreeNode]) -> [ReviewTreeNode] {
        nodes.sorted { $0.rank < $1.rank }
    }

    private static func isTest(_ file: ReviewFile) -> Bool {
        file.module?.layer == "Tests" || tested(name(file)) != nil
    }

    /// The stem of the file a test named `name` covers, as `Login` for `LoginTests.swift`,
    /// `login_tests.rs`, `login.spec.ts` or `test_login.py`; nil for a name no test has.
    static func tested(_ name: String) -> String? {
        let base = stem(name)
        if let match = base.wholeMatch(of: /(.+?)(?:[._-](?:tests?|spec)|Tests?|Spec)/) {
            return String(match.output.1)
        }
        return base.wholeMatch(of: /test_(.+)/).map { String($0.output.1) }
    }

    private static func name(_ file: ReviewFile) -> String {
        file.path.split(separator: "/").last.map(String.init) ?? file.path
    }

    private static func stem(_ name: String) -> String {
        guard let dot = name.lastIndex(of: "."), dot > name.startIndex else { return name }
        return String(name[..<dot])
    }

    private static func fileExtension(_ name: String) -> String {
        guard let dot = name.lastIndex(of: "."), dot > name.startIndex else { return "" }
        return String(name[dot...])
    }

    /// The folders every one of `folders` starts with.
    private static func common(_ folders: [[String]]) -> [String] {
        guard var shared = folders.first else { return [] }
        for folder in folders.dropFirst() {
            var same = 0
            while same < shared.count, same < folder.count, folder[same] == shared[same] { same += 1 }
            shared = Array(shared.prefix(same))
        }
        return shared
    }
}
