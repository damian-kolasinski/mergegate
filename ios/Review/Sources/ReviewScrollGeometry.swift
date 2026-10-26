struct ReviewScrollGeometry: Equatable {
    let offset: Double
    /// UIKit's own offset, which hiding the navigation bar's inset leaves where it is.
    var raw: Double = 0
    let contentHeight: Double
    let viewportHeight: Double
}
