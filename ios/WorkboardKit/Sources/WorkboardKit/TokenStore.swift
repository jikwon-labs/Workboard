// Where the device token from an invite code is kept: the Keychain, never the
// workspace copy or settings.json. One token per server address.

import Foundation
import Security

public protocol TokenStore: Sendable {
    func token(for server: String) -> String?
    func setToken(_ token: String?, for server: String)
}

public struct KeychainTokenStore: TokenStore {
    let service: String

    public init(service: String = "workboard.device-token") { self.service = service }

    func query(_ server: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: server]
    }

    public func token(for server: String) -> String? {
        var q = query(server)
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let d = out as? Data else { return nil }
        return String(data: d, encoding: .utf8)
    }

    public func setToken(_ token: String?, for server: String) {
        SecItemDelete(query(server) as CFDictionary)
        guard let token, !token.isEmpty else { return }
        var q = query(server)
        q[kSecValueData as String] = Data(token.utf8)
        q[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        SecItemAdd(q as CFDictionary, nil)
    }
}

/// For tests and command-line tools: forgets everything on exit.
public final class MemoryTokenStore: TokenStore, @unchecked Sendable {
    private var map: [String: String] = [:]
    private let lock = NSLock()
    public init() {}
    public func token(for server: String) -> String? { lock.withLock { map[server] } }
    public func setToken(_ token: String?, for server: String) { lock.withLock { map[server] = token } }
}
