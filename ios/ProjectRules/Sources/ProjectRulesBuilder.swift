import Foundation
import Networking
import SwiftUI

public enum ProjectRulesBuilder {
    @MainActor public static func build(
        hub: URL, project: String, openItem: @escaping (String) -> Void
    ) -> some View {
        RulesScene(
            model: RulesViewModel(
                service: .init(hub: hub, project: project, transport: URLSessionTransport())),
            openItem: openItem)
    }
}
