import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

final class AccountTests: XCTestCase {
    func testANewPasswordIsLongEnoughAndTypedTheSameTwice() {
        XCTAssertNil(newPasswordProblem("12345678", confirm: "12345678"))
        XCTAssertEqual(newPasswordProblem("12345678", confirm: "12345679"), "The two passwords don't match.")
        XCTAssertEqual(newPasswordProblem("1234567", confirm: "1234567"), passwordProblem("1234567"))
        XCTAssertEqual(newPasswordProblem("short", confirm: "other"), passwordProblem("short"), "too short is said first")
    }

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

    func testSignInLinkFromTheComputerIsRead() {
        // the server address as the Mac has it, percent-encoded
        let l = InviteLink.parse("workboard://link?server=https%3A%2F%2Fwb.example.com%2Fw%2Fabc123&code=K7Q2MX")
        XCTAssertEqual(l, InviteLink(server: "https://wb.example.com/w/abc123", code: "K7Q2MX", email: nil, fromComputer: true))
        XCTAssertEqual(InviteLink.parse("workboard://link?server=https://wb.example.com&code=k7q2mx")?.code, "K7Q2MX", "codes are upper case")
        XCTAssertEqual(InviteLink.parse("workboard://link?server=ws://192.168.0.5:8080&code=K7Q-2MX")?.server, "ws://192.168.0.5:8080")
        XCTAssertEqual(InviteLink.parse("workboard://link?server=http://mac-mini:8787&code=K7Q2MX")?.server, "http://mac-mini:8787")
        XCTAssertNil(InviteLink.parse("workboard://link?server=https://wb.example.com"), "no code")
        XCTAssertNil(InviteLink.parse("workboard://link?server=h&code=--"), "nothing left of the code")
        XCTAssertFalse(InviteLink.parse("workboard://join?code=X")!.fromComputer)
        // the server's address works as typed in the server field
        XCTAssertEqual(ServerAddress.websocket(l!.server!)?.absoluteString, "wss://wb.example.com/w/abc123/ws")
        XCTAssertEqual(InviteLink.find(in: "scan: workboard://link?server=https%3A%2F%2Fwb.example.com&code=K7Q2MX")?.fromComputer, true)
        // round trip
        let url = try! XCTUnwrap(l!.url)
        XCTAssertTrue(url.absoluteString.hasPrefix("workboard://link?"))
        XCTAssertEqual(InviteLink.parse(url.absoluteString), l)
    }

    func testLinkCodesAreTypedAsTheServerReadsThem() {
        XCTAssertEqual(LinkCode.normalize(" k7q-2mx "), "K7Q2MX")
        XCTAssertEqual(LinkCode.typed("k7q2mxyz"), "K7Q2MX", "at most six")
        XCTAssertEqual(LinkCode.typed("가k7"), "K7")
        XCTAssertEqual(SignInRequest.link(code: " k7q 2mx").cleaned, .link(code: "K7Q2MX"))
        XCTAssertEqual(SignInRequest.link(code: "").problem, "Type the code shown on your computer.")
        XCTAssertNil(SignInRequest.link(code: "K7Q2MX").problem)
        var h = Hello(device: "d")
        SignInRequest.link(code: "K7Q2MX").fill(&h)
        XCTAssertEqual(h.linkCode, "K7Q2MX")
        let text = ClientMessage.hello(h).text
        XCTAssertTrue(text.contains("\"link_code\":\"K7Q2MX\""), text)
        XCTAssertEqual(ClientMessage(text: text), .hello(h))
        // a link_code answer (for the Mac) is nothing the iPhone acts on
        XCTAssertEqual(ServerMessage(text: #"{"type":"link_code","code":"K7Q2MX","seconds":300}"#), .unknown("link_code"))
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
