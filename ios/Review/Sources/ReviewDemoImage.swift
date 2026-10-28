import SwiftUI
import UIKit

struct ReviewDemoImage: UIViewRepresentable {
    let image: UIImage
    let name: String
    let onTap: () -> Void
    let onActivity: () -> Void
    @Environment(\.displayScale) private var displayScale

    func makeUIView(context: Context) -> ReviewDemoImageScrollView {
        ReviewDemoImageScrollView(image: image, displayScale: displayScale)
    }

    func updateUIView(_ view: ReviewDemoImageScrollView, context: Context) {
        view.imageView.accessibilityLabel = name
        view.onTap = onTap
        view.onActivity = onActivity
    }
}

final class ReviewDemoImageScrollView: UIScrollView, UIScrollViewDelegate {
    let imageView: UIImageView
    var onTap: () -> Void = {}
    var onActivity: () -> Void = {}
    private var viewport = CGSize.zero

    init(image: UIImage, displayScale: CGFloat) {
        imageView = UIImageView(image: image)
        super.init(frame: .zero)
        // One source pixel per display pixel is 100%; small originals are never enlarged to fit.
        imageView.frame.size = CGSize(
            width: image.size.width * image.scale / displayScale,
            height: image.size.height * image.scale / displayScale)
        imageView.isAccessibilityElement = true
        imageView.accessibilityIdentifier = "demoImage"
        imageView.accessibilityHint = "Pinch or double-tap to zoom. Drag to pan when zoomed."
        imageView.accessibilityCustomActions = [
            UIAccessibilityCustomAction(name: "Zoom in", target: self, selector: #selector(zoomIn)),
            UIAccessibilityCustomAction(name: "Fit image", target: self, selector: #selector(fitImage)),
        ]
        addSubview(imageView)
        contentSize = imageView.bounds.size
        accessibilityIdentifier = "demoImageCanvas"
        delegate = self
        contentInsetAdjustmentBehavior = .never
        backgroundColor = .clear
        let doubleTap = UITapGestureRecognizer(target: self, action: #selector(doubleTapped(_:)))
        doubleTap.numberOfTapsRequired = 2
        let tap = UITapGestureRecognizer(target: self, action: #selector(tapped))
        tap.require(toFail: doubleTap)
        addGestureRecognizer(tap)
        addGestureRecognizer(doubleTap)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func layoutSubviews() {
        super.layoutSubviews()
        if bounds.size != viewport, bounds.width > 0, bounds.height > 0 {
            let wasFitted = viewport == .zero || abs(zoomScale - minimumZoomScale) < 0.001
            viewport = bounds.size
            let fit = min(1, bounds.width / imageView.bounds.width, bounds.height / imageView.bounds.height)
            minimumZoomScale = fit
            maximumZoomScale = max(4, fit * 8)
            setZoomScale(wasFitted ? fit : min(maximumZoomScale, max(fit, zoomScale)), animated: false)
        }
        centerImage()
    }

    func viewForZooming(in scrollView: UIScrollView) -> UIView? { imageView }

    func scrollViewDidZoom(_ scrollView: UIScrollView) {
        centerImage()
        onActivity()
    }

    func scrollViewWillBeginDragging(_ scrollView: UIScrollView) { onActivity() }

    private func centerImage() {
        let inset = UIEdgeInsets(
            top: max(0, (bounds.height - contentSize.height) / 2),
            left: max(0, (bounds.width - contentSize.width) / 2), bottom: 0, right: 0)
        if contentInset != inset { contentInset = inset }
        imageView.accessibilityValue = "\(Int((zoomScale * 100).rounded())) percent"
    }

    @objc private func tapped() { onTap() }

    @objc private func doubleTapped(_ recognizer: UITapGestureRecognizer) {
        onActivity()
        if zoomScale > minimumZoomScale + 0.001 {
            setZoomScale(minimumZoomScale, animated: true)
        } else {
            let scale = min(maximumZoomScale, max(1, minimumZoomScale * 2))
            let point = recognizer.location(in: imageView)
            let size = CGSize(width: bounds.width / scale, height: bounds.height / scale)
            zoom(to: CGRect(x: point.x - size.width / 2, y: point.y - size.height / 2,
                            width: size.width, height: size.height), animated: true)
        }
    }

    @objc private func zoomIn() -> Bool {
        onActivity()
        setZoomScale(min(maximumZoomScale, zoomScale * 2), animated: true)
        return true
    }

    @objc private func fitImage() -> Bool {
        onActivity()
        setZoomScale(minimumZoomScale, animated: true)
        return true
    }
}
