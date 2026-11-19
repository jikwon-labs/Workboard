import XCTest
@testable import WorkboardKit

final class DocFormatTests: XCTestCase {
    func sel(_ at: Int, _ len: Int = 0) -> NSRange { NSRange(location: at, length: len) }

    func testBoldWrapsTheSelectionAndKeepsItSelected() {
        let (t, s) = DocFormat.apply(.bold, to: "make it big", selection: sel(8, 3))
        XCTAssertEqual(t, "make it **big**")
        XCTAssertEqual(s, sel(10, 3))
    }

    func testBoldWithNothingSelectedPutsTheCaretInsideThePair() {
        let (t, s) = DocFormat.apply(.bold, to: "a ", selection: sel(2))
        XCTAssertEqual(t, "a ****")
        XCTAssertEqual(s, sel(4))
    }

    func testHeadingReplacesTheLinesPrefixAndTogglesOff() {
        let (t1, s1) = DocFormat.apply(.heading1, to: "one\n## two", selection: sel(7))
        XCTAssertEqual(t1, "one\n# two")
        XCTAssertEqual(s1, sel(6))
        let (t2, _) = DocFormat.apply(.heading1, to: t1, selection: s1)
        XCTAssertEqual(t2, "one\ntwo", "the same button again makes it plain")
    }

    func testChecklistAndListPrefixesMatchTheMac() {
        XCTAssertEqual(DocFormat.apply(.checklist, to: "buy milk", selection: sel(0)).0, "- [ ] buy milk")
        XCTAssertEqual(DocFormat.apply(.bullet, to: "- [ ] buy milk", selection: sel(8)).0, "- buy milk")
        XCTAssertEqual(DocFormat.apply(.numbered, to: "step", selection: sel(4)).0, "1. step")
        XCTAssertEqual(DocFormat.apply(.quote, to: "said", selection: sel(0)).0, "> said")
    }

    func testCodeBlockAndDivider() {
        let (t, s) = DocFormat.apply(.codeBlock, to: "", selection: sel(0))
        XCTAssertEqual(t, "```\n\n```")
        XCTAssertEqual(s, sel(4))
        XCTAssertEqual(DocFormat.apply(.divider, to: "end", selection: sel(3)).0, "---\nend")
    }

    func testLinkPutsTheCaretOnTheAddress() {
        let (t, s) = DocFormat.apply(.link, to: "see site", selection: sel(4, 4))
        XCTAssertEqual(t, "see [site](https://)")
        XCTAssertEqual(s, sel(19))
        XCTAssertEqual(DocFormat.apply(.link, to: "", selection: sel(0)).1, sel(1))
    }

    func testKoreanTextCountsInUTF16() {
        let (t, s) = DocFormat.apply(.italic, to: "회의 메모", selection: sel(3, 2))
        XCTAssertEqual(t, "회의 *메모*")
        XCTAssertEqual(s, sel(4, 2))
    }

    func testReturnContinuesAndEndsLists() {
        XCTAssertEqual(DocFormat.newline(in: "- a", at: 3).0, "- a\n- ")
        XCTAssertEqual(DocFormat.newline(in: "- [x] done", at: 10).0, "- [x] done\n- [ ] ")
        XCTAssertEqual(DocFormat.newline(in: "9. nine", at: 7).0, "9. nine\n10. ")
        XCTAssertEqual(DocFormat.newline(in: "  - nested", at: 10).0, "  - nested\n  - ")
        let (t, c) = DocFormat.newline(in: "- a\n- ", at: 6)
        XCTAssertEqual(t, "- a\n", "an empty item ends the list")
        XCTAssertEqual(c, 4)
        XCTAssertEqual(DocFormat.newline(in: "plain", at: 5).0, "plain\n")
    }

    func testRunsFindHeadingsMarkersAndInlineStyles() {
        let text = "# Title\n**b** and [x](https://a.b)\n- [x] done\n```\ncode\n```"
        let runs = DocFormat.runs(text)
        let ns = text as NSString
        func styles(at needle: String) -> [DocFormat.Style] {
            let r = ns.range(of: needle)
            return runs.filter { NSLocationInRange(r.location, $0.range) }.map(\.style)
        }
        XCTAssertEqual(styles(at: "# "), [.heading(1), .marker])
        XCTAssertEqual(styles(at: "Title"), [.heading(1)])
        XCTAssertEqual(styles(at: "b**"), [.bold])
        XCTAssertEqual(styles(at: "x]"), [.link])
        XCTAssertEqual(styles(at: "https"), [.marker])
        XCTAssertEqual(styles(at: "done"), [.done])
        XCTAssertEqual(styles(at: "code"), [.codeBlock])
    }
}
