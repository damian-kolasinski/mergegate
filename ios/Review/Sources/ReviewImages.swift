import ImageIO
import UIKit

struct ReviewPixelDifference: Sendable {
    let image: UIImage
    let changed: Int
    let total: Int
}

actor ReviewImages {
    static let shared = ReviewImages()
    private let maxPixels = 16_000_000

    func decode(_ data: Data) throws -> UIImage {
        guard data.count <= 64 * 1024 * 1024,
            let source = CGImageSourceCreateWithData(data as CFData, nil),
            let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
            let width = properties[kCGImagePropertyPixelWidth] as? Int,
            let height = properties[kCGImagePropertyPixelHeight] as? Int,
            width > 0, height > 0, width <= maxPixels / height
        else {
            throw ImageIssue(
                message:
                    "This image exceeds the 64 MB or 16 million pixel preview limit, or cannot be decoded."
            )
        }
        guard let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
            throw ImageIssue(message: "This image format cannot be decoded on this device.")
        }
        return UIImage(cgImage: image)
    }

    func compare(_ before: UIImage, _ after: UIImage) -> ReviewPixelDifference? {
        guard let a = before.cgImage, let b = after.cgImage else { return nil }
        let width = max(a.width, b.width)
        let height = max(a.height, b.height)
        guard width <= maxPixels / height,
            let old = pixels(a), let new = pixels(b)
        else { return nil }
        var output = [UInt8](repeating: 0, count: width * height * 4)
        var changed = 0
        for y in 0..<height {
            for x in 0..<width {
                let target = (y * width + x) * 4
                let ai = (y * a.width + x) * 4
                let bi = (y * b.width + x) * 4
                if x >= a.width || y >= a.height || x >= b.width || y >= b.height
                    || old[ai..<ai + 4] != new[bi..<bi + 4]
                {
                    output.replaceSubrange(target..<target + 4, with: [239, 58, 111, 255])
                    changed += 1
                } else {
                    let grey = UInt8((Int(new[bi]) + Int(new[bi + 1]) + Int(new[bi + 2])) / 12)
                    output.replaceSubrange(
                        target..<target + 4, with: [grey, grey, grey, new[bi + 3] / 4])
                }
            }
        }
        guard let provider = CGDataProvider(data: Data(output) as CFData),
            let image = CGImage(
                width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32,
                bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
                provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)
        else { return nil }
        return .init(image: UIImage(cgImage: image), changed: changed, total: width * height)
    }
    private func pixels(_ image: CGImage) -> [UInt8]? {
        var data = [UInt8](repeating: 0, count: image.width * image.height * 4)
        // SAFETY: The context uses exactly the scoped RGBA buffer and never escapes it.
        let rendered = data.withUnsafeMutableBytes { bytes in
            guard
                let context = CGContext(
                    data: bytes.baseAddress, width: image.width, height: image.height,
                    bitsPerComponent: 8, bytesPerRow: image.width * 4,
                    space: CGColorSpaceCreateDeviceRGB(),
                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
            else { return false }
            context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
            return true
        }
        return rendered ? data : nil
    }
}

private struct ImageIssue: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}
