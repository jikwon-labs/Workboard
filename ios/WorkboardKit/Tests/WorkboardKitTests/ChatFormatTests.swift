import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// Chat formatting and link previews, as the Mac app reads them
/// (src/app/chatfmt.rs, src/linkpreview.rs).
final class ChatFormatTests: XCTestCase {
    func testBlocksQuotesListsAndCode() {
        let b = ChatFormat.blocks("look:\nsecond\n```\nfn main() {}\n  x\n```\n> quoted *bit*\n- one\n* two\n```let y = 1```")
        XCTAssertEqual(b, [.text("look:\nsecond"), .code("fn main() {}\n  x"), .quote("quoted *bit*"), .bullet("one"), .bullet("two"), .code("let y = 1")])
    }

    func testInlineMarksAndLinks() {
        let a = ChatFormat.inline("**Ship** it *now* ~~not~~ `cargo` https://example.com/x.")
        XCTAssertEqual(String(a.characters), "Ship it now not cargo https://example.com/x.")
        XCTAssertTrue(a.runs.contains { $0.inlinePresentationIntent?.contains(.stronglyEmphasized) == true && String(a[$0.range].characters) == "Ship" })
        XCTAssertTrue(a.runs.contains { $0.inlinePresentationIntent?.contains(.code) == true })
        XCTAssertEqual(a.runs.compactMap(\.link).first?.absoluteString, "https://example.com/x")
        XCTAssertEqual(String(ChatFormat.inline("2 * 3 * 4").characters), "2 * 3 * 4", "marks with spaces inside stay")
        XCTAssertEqual(String(ChatFormat.inline("fine ¯\\_(ツ)_/¯ and snake_case_name").characters), "fine ¯\\_(ツ)_/¯ and snake_case_name",
                       "no \\ escapes or _italic_, as on the Mac")
        XCTAssertEqual(String(ChatFormat.inline("`a_b\\c` x").characters), "a_b\\c x")
        XCTAssertEqual(ChatFormat.inline("see https://ex.example/a_b").runs.compactMap(\.link).first?.absoluteString, "https://ex.example/a_b")
        XCTAssertEqual(ChatFormat.firstURL("see `https://no.example` and [doc](https://d.example/a)"), "https://d.example/a")
        XCTAssertEqual(ChatFormat.firstURL("```\nhttps://code.example\n```\nplain"), nil)
    }

    func testWrapAndUnwrapLikeTheMac() {
        var (t, a, b) = ChatFormat.wrap("make this bold", 10, 14, mark: "**")
        XCTAssertEqual(t, "make this **bold**"); XCTAssertEqual([a, b], [12, 16])
        (t, a, b) = ChatFormat.wrap(t, a, b, mark: "*")
        XCTAssertEqual(t, "make this ***bold***", "italic goes inside the bold")
        (t, a, b) = ChatFormat.wrap(t, a, b, mark: "*")
        XCTAssertEqual(t, "make this **bold**")
        (t, a, b) = ChatFormat.wrap(t, a, b, mark: "**")
        XCTAssertEqual(t, "make this bold"); XCTAssertEqual([a, b], [10, 14])
        XCTAssertEqual(ChatFormat.wrap("한글 ", 3, 3, mark: "*").0, "한글 **")
        XCTAssertEqual(ChatFormat.wrap("a\nb", 0, 3, mark: "`").0, "```\na\nb\n```")
    }

    func testPreviewFromAPage() {
        let html = """
        <html><head><title>Plain title</title>
        <meta property="og:title" content="Launch &amp; ship">
        <meta name='description' content='The plan
           for Friday'>
        <META PROPERTY="og:site_name" CONTENT="Example Co"/>
        </head><body><meta property="og:title" content="not this"></body>
        """
        XCTAssertEqual(LinkPreviews.fromHTML(url: "https://www.example.com/a", html: html),
                       ["url": "https://www.example.com/a", "title": "Launch & ship", "description": "The plan for Friday", "site": "Example Co"])
        let p = LinkPreviews.fromHTML(url: "https://www.rust-lang.org/learn", html: "<title> Learn &#8212; Rust </title>")
        XCTAssertEqual(p?["title"], "Learn \u{2014} Rust")
        XCTAssertEqual(p?["site"], "rust-lang.org")
        XCTAssertNil(LinkPreviews.fromHTML(url: "https://x.example", html: "<body>nothing</body>"))
    }

    func testPreviewIsKeptRemovedAndDroppedWhenTheAddressChanges() {
        var d = SampleWorkspace.make()
        Edits.sendMessage(&d, channel: 301, text: "plan at https://example.com/plan", me: 1, id: 900, ts: 10)
        let p: JSON = ["url": "https://example.com/plan", "title": "Plan", "description": "", "site": "example.com"]
        XCTAssertTrue(Edits.setPreview(&d, channel: 301, message: 900, preview: p, me: 1))
        func msg() -> MessageView { WorkspaceView(d).channels.first { $0.id == 301 }!.messages.first { $0.id == 900 }! }
        XCTAssertEqual(msg().preview?.title, "Plan")
        // another address: the old preview goes
        Edits.editMessage(&d, channel: 301, message: 900, text: "plan at https://example.com/other", me: 1, ts: 11)
        XCTAssertNil(msg().preview)
        // a stale fetch for the old address doesn't come back
        Edits.setPreview(&d, channel: 301, message: 900, preview: p, me: 1)
        XCTAssertNil(msg().preview)
        var q = p; q["url"] = "https://example.com/other"
        Edits.setPreview(&d, channel: 301, message: 900, preview: q, me: 1)
        XCTAssertNotNil(msg().preview)
        Edits.removePreview(&d, channel: 301, message: 900, me: 2)
        XCTAssertNotNil(msg().preview, "only its writer")
        Edits.removePreview(&d, channel: 301, message: 900, me: 1)
        XCTAssertNil(msg().preview)
    }
}
