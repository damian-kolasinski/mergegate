import DiffDocumentInterface
import Testing
import UIKit

@testable import CodeStyling

struct CodeStylingTests {
    @Test @MainActor func unicodeRangesUnknownTagsAndMalformedRangesKeepSourceReadable() {
        let source = "é 🐙 let value"
        let font = UIFont.monospacedSystemFont(ofSize: 13, weight: .regular)
        let styled = NativeCodeStyling().apply(
            spans: [
                CodeSpan(start: 5, length: 3, tag: "keyword"),
                CodeSpan(start: 2, length: 1, tag: "string"),  // Half of 🐙.
                CodeSpan(start: 100, length: 2, tag: "keyword"),
                CodeSpan(start: -1, length: 2, tag: "keyword"),
                CodeSpan(start: 9, length: Int.max, tag: "keyword"),
                CodeSpan(start: 9, length: 5, tag: "future_tag"),
            ], source: source, font: font, traits: UITraitCollection(userInterfaceStyle: .light))
        #expect(styled.string == source)
        #expect(
            styled.attribute(.foregroundColor, at: 5, effectiveRange: nil) as? UIColor
                == SyntaxPalette.color("keyword", traits: .init(userInterfaceStyle: .light)))
        #expect(styled.attribute(.foregroundColor, at: 2, effectiveRange: nil) == nil)
        #expect(styled.attribute(.foregroundColor, at: 9, effectiveRange: nil) == nil)
    }

    @Test @MainActor func themeAndTextTraitsMatchTheSharedSyntaxPalette() throws {
        let styler = NativeCodeStyling()
        let font = UIFont.monospacedSystemFont(ofSize: 32, weight: .regular)
        for style in [UIUserInterfaceStyle.light, .dark] {
            let traits = UITraitCollection(userInterfaceStyle: style)
            let result = styler.apply(
                spans: [
                    CodeSpan(start: 0, length: 1, tag: "emphasis"),
                    CodeSpan(start: 1, length: 1, tag: "strong"), CodeSpan(start: 2, length: 1, tag: "link"),
                    CodeSpan(start: 3, length: 1, tag: "heading"),
                ],
                source: "abcd", font: font, traits: traits)
            let italic = try #require(result.attribute(.font, at: 0, effectiveRange: nil) as? UIFont)
            #expect(italic.fontDescriptor.symbolicTraits.contains(.traitItalic))
            let strong = try #require(result.attribute(.font, at: 1, effectiveRange: nil) as? UIFont)
            #expect(strong.pointSize == font.pointSize)
            #expect(
                result.attribute(.underlineStyle, at: 2, effectiveRange: nil) as? Int
                    == NSUnderlineStyle.single.rawValue)
            #expect(result.attribute(.font, at: 3, effectiveRange: nil) as? UIFont == strong)
            #expect(
                result.attribute(.foregroundColor, at: 2, effectiveRange: nil) as? UIColor
                    == SyntaxPalette.color("link", traits: traits))
        }
        #expect(SyntaxPalette.color("variable", traits: .init()) == nil)
        #expect(
            SyntaxPalette.color("keyword", traits: .init(userInterfaceStyle: .light))
                != SyntaxPalette.color("keyword", traits: .init(userInterfaceStyle: .dark)))
    }
}
