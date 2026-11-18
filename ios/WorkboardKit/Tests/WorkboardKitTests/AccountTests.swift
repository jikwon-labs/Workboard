import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

final class AccountTests: XCTestCase {
    func testInviteLinkIsRead() {
        let l = InviteLink.parse("workboard://join?server=mac-mini.tail1.ts.net:8787&code=ABCD-EFGH-JKMN&email=sam%2Bwork%40example.com")
        XCTAssertEqual(l, InviteLink(server: "ws://mac-mini.tail1.ts.net:8787", code: "ABCD-EFGH-JKMN", email: "sam+work@example.com"))
        XCTAssertEqual(InviteLink.parse("workboard://join?server=10.0.0.5&code=X")?.server, "ws://10.0.0.5")
        XCTAssertEqual(InviteLink.parse("workboard://join?code=X"), InviteLink(server: nil, code: "X", email: nil))
        XCTAssertNil(InviteLink.parse("workboard://join?server=h"), "no code")
        XCTAssertNil(InviteLink.parse("workboard://other?code=X"))
        XCTAssertNil(InviteLink.parse("ABCD-EFGH-JKMN"))
        // the server's web page link
        XCTAssertEqual(InviteLink.parse("http://mac-mini:8787/join?code=ABCD-EFGH-JKMN&email=a%40b.c"),
                       InviteLink(server: "ws://mac-mini:8787", code: "ABCD-EFGH-JKMN", email: "a@b.c"))
        XCTAssertEqual(InviteLink.parse("https://wb.example.com/join?code=C")?.server, "wss://wb.example.com")
        XCTAssertNil(InviteLink.parse("https://example.com/pricing?code=C"))
    }

    func testInviteLinkIsFoundInAPastedEmail() {
        let mail = """
        Hi Sam,

        Jamie invited you to Sample Team on Workboard.
        Open this on your iPhone: <workboard://join?server=mac-mini.tail1.ts.net%3A8787&code=QWER-TYUP-ASDF&email=sam%40example.com>.
        Or visit https://example.com/help for help.
        """
        XCTAssertEqual(InviteLink.find(in: mail), InviteLink(server: "ws://mac-mini.tail1.ts.net:8787", code: "QWER-TYUP-ASDF", email: "sam@example.com"))
        XCTAssertEqual(InviteLink.find(in: "join here: http://10.1.2.3:8787/join?code=AAAA-BBBB-CCCC (7 days)")?.code, "AAAA-BBBB-CCCC")
        XCTAssertNil(InviteLink.find(in: "AAAA-BBBB-CCCC"))
        XCTAssertNil(InviteLink.find(in: "see https://example.com/x"))
    }

    func testInviteLinkRoundTrips() {
        let l = InviteLink(server: "ws://host:8787", code: "ABCD-EFGH-JKMN", email: "a+b@c.d")
        let url = try! XCTUnwrap(l.url)
        XCTAssertTrue(url.absoluteString.hasPrefix("workboard://join?server=host:8787&code=ABCD-EFGH-JKMN"))
        XCTAssertEqual(InviteLink.parse(url.absoluteString), l)
    }

    func testSignInRequestsAreCheckedBeforeSending() {
        XCTAssertNotNil(SignInRequest.password(email: "", password: "x").problem)
        XCTAssertNil(SignInRequest.password(email: "a@b.c", password: "x").problem, "old passwords are the server's business")
        XCTAssertNotNil(SignInRequest.invite(code: "C", email: nil, newPassword: "short").problem)
        XCTAssertNil(SignInRequest.invite(code: "C", email: nil, newPassword: "12345678").problem)
        XCTAssertNil(SignInRequest.invite(code: "C", email: nil, newPassword: nil).problem)
        XCTAssertNotNil(SignInRequest.reset(email: "a@b.c", code: "", newPassword: "12345678").problem)
        XCTAssertNotNil(SignInRequest.reset(email: "a@b.c", code: "R", newPassword: "1234567").problem)
        var h = Hello(device: "d")
        SignInRequest.invite(code: "C", email: "", newPassword: "12345678").fill(&h)
        XCTAssertNil(h.email, "an empty email is left out, so the server uses the member record")
    }
}
