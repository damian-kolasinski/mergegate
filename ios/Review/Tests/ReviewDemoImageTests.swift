import Testing
import UIKit

@testable import Review

struct ReviewDemoImageTests {
    @Test @MainActor func imageFitsWithoutStretchingAndZoomCanPanPastTheViewport() {
        let view = reader(size: CGSize(width: 1200, height: 600))
        view.frame = CGRect(x: 0, y: 0, width: 300, height: 500)
        view.layoutIfNeeded()
        #expect(view.zoomScale == 0.75)
        #expect(view.imageView.frame.size == CGSize(width: 300, height: 150))
        #expect(view.contentInset.top == 175)
        view.setZoomScale(1.5, animated: false)
        view.layoutIfNeeded()
        #expect(view.contentSize.width == 600)
        #expect(view.imageView.frame.width / view.imageView.frame.height == 2)
        view.setContentOffset(CGPoint(x: 100, y: 0), animated: false)
        view.layoutIfNeeded()
        #expect(view.contentOffset.x == 100)
    }

    @Test @MainActor func smallImagesStayAtNaturalPixelScaleAndRotationRefitsLargeImages() {
        let small = reader(size: CGSize(width: 120, height: 60))
        small.frame = CGRect(x: 0, y: 0, width: 300, height: 500)
        small.layoutIfNeeded()
        #expect(small.zoomScale == 1)
        #expect(small.imageView.frame.size == CGSize(width: 40, height: 20))
        #expect(small.contentInset.left == 130)
        let large = reader(size: CGSize(width: 1200, height: 600))
        large.frame = CGRect(x: 0, y: 0, width: 300, height: 500)
        large.layoutIfNeeded()
        large.frame.size = CGSize(width: 600, height: 300)
        large.layoutIfNeeded()
        #expect(large.zoomScale == 1)
        #expect(large.imageView.frame.size == CGSize(width: 400, height: 200))
    }

    @MainActor private func reader(size: CGSize) -> ReviewDemoImageScrollView {
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        let image = UIGraphicsImageRenderer(size: size, format: format).image { context in
            UIColor.blue.setFill()
            context.fill(CGRect(origin: .zero, size: size))
        }
        return ReviewDemoImageScrollView(image: image, displayScale: 3)
    }
}
