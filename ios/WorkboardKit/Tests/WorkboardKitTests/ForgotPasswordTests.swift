import XCTest
@testable import WorkboardKit

/// Answers every request with a fixed status and body.
private final class CannedAnswer: URLProtocol {
    nonisolated(unsafe) static var status = 200
    nonisolated(unsafe) static var body = ""

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        let resp = HTTPURLResponse(url: request.url!, statusCode: Self.status, httpVersion: nil, headerFields: nil)!
        client?.urlProtocol(self, didReceive: resp, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data(Self.body.utf8))
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
}

/// What "Forgot password" says for each answer a server (or the registry in
/// front of a workspace) gives.
final class ForgotPasswordTests: XCTestCase {
    func answer(_ status: Int, _ body: String) async -> Result<Bool, AuthError> {
        CannedAnswer.status = status
        CannedAnswer.body = body
        let cfg = URLSessionConfiguration.ephemeral
        cfg.protocolClasses = [CannedAnswer.self]
        do {
            return .success(try await AuthAPI.forgot(server: "macmini/w/ab12", email: "a@b.io", session: URLSession(configuration: cfg)))
        } catch let e as AuthError {
            return .failure(e)
        } catch {
            return .failure(.failed("\(error)"))
        }
    }

    func testTheServersReasonIsShown() async {
        let ok = await answer(200, #"{"ok":true,"mail":true}"#)
        XCTAssertEqual(ok, .success(true))
        let gone = await answer(410, #"{"error":"This workspace was deleted from the server by its owner."}"#)
        XCTAssertEqual(gone, .failure(.failed("This workspace was deleted from the server by its owner.")))
        let missing = await answer(404, #"{"error":"There's no such workspace on this server."}"#)
        XCTAssertEqual(missing, .failure(.failed("There's no such workspace on this server.")))
        let older = await answer(403, #"{"message":"No."}"#)
        XCTAssertEqual(older, .failure(.failed("No.")))
        // a server from before /auth/forgot: no reason, just a 404
        let old = await answer(404, "Not Found")
        XCTAssertEqual(old, .failure(.failed("This server can't reset passwords. Ask the owner or an admin for a new invite code.")))
        let odd = await answer(500, "")
        XCTAssertEqual(odd, .failure(.failed("The server answered 500.")))
    }
}
