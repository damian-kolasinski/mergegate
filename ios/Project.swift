import ProjectDescription

let settings: Settings = .settings(base: [
    "SWIFT_VERSION": "6.0",
    "SWIFT_STRICT_CONCURRENCY": "complete",
    "IPHONEOS_DEPLOYMENT_TARGET": "26.0",
    "CODE_SIGN_STYLE": "Automatic",
])

func module(_ name: String, dependencies: [String] = [], appearance: Bool = false) -> Target {
    .target(
        name: name, destinations: [.iPhone, .iPad], product: .framework,
        bundleId: "dev.mergegate.\(name)", deploymentTargets: .iOS("26.0"),
        sources: appearance ? ["\(name)/Sources/**", "Appearance/**"] : ["\(name)/Sources/**"],
        dependencies: dependencies.map { .target(name: $0) }
    )
}
func tests(_ name: String) -> Target {
    .target(
        name: "\(name)Tests", destinations: [.iPhone, .iPad], product: .unitTests,
        bundleId: "dev.mergegate.\(name)Tests", deploymentTargets: .iOS("26.0"),
        sources: ["\(name)/Tests/**"],
        resources: name == "DiffView" ? [.folderReference(path: "DiffView/Tests/Goldens")] : [],
        dependencies: [.target(name: name)]
            + (name == "DiffView" ? [.target(name: "CodeStyling")] : [])
    )
}
let project = Project(
    name: "Mergegate", settings: settings,
    targets: [
        .target(
            name: "Mergegate", destinations: [.iPhone, .iPad], product: .app,
            bundleId: "dev.mergegate.ios", deploymentTargets: .iOS("26.0"),
            infoPlist: .extendingDefault(with: [
                "CFBundleDisplayName": "mergegate",
                "UILaunchScreen": [:],
                "UIAppFonts": ["DMSans.ttf"],
                "NSLocalNetworkUsageDescription":
                    "Connect to your mergegate hub through Tailscale.",
                "NSAppTransportSecurity": [
                    "NSAllowsLocalNetworking": true,
                    "NSExceptionDomains": [
                        "100.64.0.0/10": ["NSExceptionAllowsInsecureHTTPLoads": true],
                        "fd7a:115c:a1e0::/48": ["NSExceptionAllowsInsecureHTTPLoads": true],
                        "ts.net": [
                            "NSIncludesSubdomains": true,
                            "NSExceptionAllowsInsecureHTTPLoads": true,
                        ],
                    ],
                ],
            ]),
            sources: ["App/**"], resources: ["Resources/**"],
            dependencies: [
                .target(name: "HubConnection"), .target(name: "Inbox"), .target(name: "Review"),
                .target(name: "CodeStyling"), .target(name: "Conversation"),
                .target(name: "ReviewSubmission"), .target(name: "ProjectRules"),
            ]
        ),
        .target(
            name: "HubConnectionInterface", destinations: [.iPhone, .iPad], product: .framework,
            bundleId: "dev.mergegate.HubConnectionInterface", deploymentTargets: .iOS("26.0"),
            sources: ["HubConnection/Interface/**"]
        ),
        module("Networking"),
        module("ProjectRules", dependencies: ["Networking"], appearance: true),
        tests("ProjectRules"),
        module(
            "HubConnection", dependencies: ["HubConnectionInterface", "Networking"],
            appearance: true),
        module(
            "Inbox",
            dependencies: ["HubConnectionInterface", "ConversationInterface", "Networking"],
            appearance: true),
        .target(
            name: "DiffDocumentInterface", destinations: [.iPhone, .iPad], product: .framework,
            bundleId: "dev.mergegate.DiffDocumentInterface", deploymentTargets: .iOS("26.0"),
            sources: ["DiffDocument/Interface/**"]
        ),
        .target(
            name: "ConversationInterface", destinations: [.iPhone, .iPad], product: .framework,
            bundleId: "dev.mergegate.ConversationInterface", deploymentTargets: .iOS("26.0"),
            sources: ["Conversation/Interface/**"],
            dependencies: [.target(name: "DiffDocumentInterface")]
        ),
        module(
            "Conversation",
            dependencies: ["ConversationInterface", "DiffDocumentInterface", "Networking"],
            appearance: true),
        module("DiffDocument", dependencies: ["DiffDocumentInterface"]),
        .target(
            name: "CodeStylingInterface", destinations: [.iPhone, .iPad], product: .framework,
            bundleId: "dev.mergegate.CodeStylingInterface", deploymentTargets: .iOS("26.0"),
            sources: ["CodeStyling/Interface/**"],
            dependencies: [.target(name: "DiffDocumentInterface")]
        ),
        module("CodeStyling", dependencies: ["CodeStylingInterface", "DiffDocumentInterface"]),
        module(
            "DiffView", dependencies: ["DiffDocumentInterface", "CodeStylingInterface"],
            appearance: true),
        module(
            "Review",
            dependencies: [
                "HubConnectionInterface", "Networking", "DiffDocument", "DiffDocumentInterface",
                "DiffView",
                "CodeStylingInterface", "ConversationInterface",
            ], appearance: true),
        module(
            "ReviewSubmission", dependencies: ["ConversationInterface", "Networking"],
            appearance: true),
        tests("ReviewSubmission"), tests("Conversation"), tests("CodeStyling"),
        tests("DiffDocument"),
        tests("DiffView"),
        tests("Review"),
        tests("HubConnection"), tests("Inbox"), tests("Networking"),
        .target(
            name: "MergegateUITests", destinations: [.iPhone, .iPad], product: .uiTests,
            bundleId: "dev.mergegate.ios.UITests", deploymentTargets: .iOS("26.0"),
            sources: ["Tests/**"], dependencies: [.target(name: "Mergegate")]
        ),
    ],
    schemes: [
        .scheme(
            name: "Mergegate", buildAction: .buildAction(targets: ["Mergegate"]),
            testAction: .targets(
                [
                    "ProjectRulesTests", "ReviewSubmissionTests", "ConversationTests",
                    "CodeStylingTests",
                    "HubConnectionTests",
                    "InboxTests",
                    "NetworkingTests",
                    "DiffDocumentTests", "DiffViewTests", "ReviewTests", "MergegateUITests",
                ],
                arguments: .arguments(environmentVariables: [
                    "MG_IOS_TEST_HUB_URL": "$(MG_IOS_TEST_HUB_URL)",
                    "MG_IOS_TEST_PARITY": "$(MG_IOS_TEST_PARITY)",
                    "MG_IOS_TEST_DEMO": "$(MG_IOS_TEST_DEMO)",
                    "MG_IOS_TEST_TRIAGE": "$(MG_IOS_TEST_TRIAGE)",
                    "MG_IOS_TEST_REVIEW": "$(MG_IOS_TEST_REVIEW)",
                    "MG_IOS_TEST_SOURCE": "$(MG_IOS_TEST_SOURCE)",
                    "MG_IOS_TEST_HIGHLIGHT": "$(MG_IOS_TEST_HIGHLIGHT)",
                    "MG_IOS_TEST_VIEWED": "$(MG_IOS_TEST_VIEWED)",
                    "MG_IOS_TEST_DRAFTS": "$(MG_IOS_TEST_DRAFTS)",
                    "MG_IOS_TEST_CONVERSATIONS": "$(MG_IOS_TEST_CONVERSATIONS)",
                    "MG_IOS_TEST_REREVIEW": "$(MG_IOS_TEST_REREVIEW)",
                    "MG_IOS_TEST_APPROVAL": "$(MG_IOS_TEST_APPROVAL)",
                    "MG_IOS_TEST_OFFLINE": "$(MG_IOS_TEST_OFFLINE)",
                    "MG_IOS_TEST_LIVE_UPDATES": "$(MG_IOS_TEST_LIVE_UPDATES)",
                    "MG_IOS_TEST_CONTROL_URL": "$(MG_IOS_TEST_CONTROL_URL)",
                    "MG_IOS_TEST_DECISIONS": "$(MG_IOS_TEST_DECISIONS)",
                    "MG_IOS_TEST_FEEDBACK": "$(MG_IOS_TEST_FEEDBACK)",
                    "MG_IOS_TEST_REQUEST_CHANGES": "$(MG_IOS_TEST_REQUEST_CHANGES)",
                    "MG_IOS_TEST_INLINE": "$(MG_IOS_TEST_INLINE)",
                ]), configuration: .debug, expandVariableFromTarget: "Mergegate"
            ), runAction: .runAction(configuration: .debug)
        )
    ], resourceSynthesizers: []
)
