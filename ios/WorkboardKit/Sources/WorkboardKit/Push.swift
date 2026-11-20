// Push notifications: this install's APNs token sent to the server, and a
// server told to stop notifying this device (an empty push_token), retried
// until it is heard.
//
// Every workspace kept on this iPhone keeps its registration, so news from
// one that isn't open still comes as a push: the token goes with this
// iPhone's name for the workspace (`pushKey`), which the server puts in each
// notification as "ws" so a tap opens that workspace (`savedWorkspace(pushKey:)`).
// Signing out and removing a kept workspace stop its pushes.

import Foundation

extension SyncClient {
    /// The APNs token for this install; sent now if connected, else after the next welcome.
    public func setPushToken(_ token: String, env: String) {
        settings.pushToken = token
        settings.pushEnv = env
        storage.save(settings)
        if state.isConnected, let ws = socket { sendPushToken(on: ws) }
    }

    /// This phone's notification choices for the server's pushes
    /// (NotifyPrefs.forServer): sent with the token, and again when they change.
    public func setPushPrefs(_ prefs: JSON) {
        guard prefs != pushPrefs else { return }
        pushPrefs = prefs
        if state.isConnected, let ws = socket { sendPushToken(on: ws) }
    }

    func sendPushToken(on ws: URLSessionWebSocketTask) {
        guard let t = settings.pushToken else { return }
        send(.pushToken(token: t, platform: "ios", env: settings.pushEnv, workspace: pushKey, prefs: pushPrefs), on: ws)
        settings.pushSentTo = settings.server
        storage.save(settings)
    }

    /// This iPhone's name for the open workspace in notifications ("ws").
    public var pushKey: String { ServerAddress.folderKey(settings.server) }

    /// The kept workspace a notification with `key` ("ws") is from; nil
    /// when it is the open one, an older server sent none, or it isn't kept.
    public func savedWorkspace(pushKey key: String?) -> SavedWorkspace? {
        guard let key, !key.isEmpty, key != pushKey else { return nil }
        return savedWorkspaces.first { $0.folder == key }
    }

    /// Where a device token waits until its server is told to stop pushes.
    nonisolated static func unregisterKey(_ server: String) -> String { "push-unregister|" + server }

    /// Tell `server` to stop notifying this device (push_token with an
    /// empty token, which the server takes as "forget it") and forget that
    /// it was registered there. Over the open connection when there is
    /// one; else, and when that fails, over a short connection of its own
    /// signed in with `token`, tried again on every connect until it works.
    func stopPushes(from server: String, token: String?) {
        guard let sent = settings.pushSentTo, ServerAddress.same(sent, server) else { return }
        settings.pushSentTo = nil
        storage.save(settings)
        guard state.isConnected, let ws = socket, ServerAddress.same(settings.server, server) else {
            return queueUnregister(server, token: token)
        }
        // keep this socket out of closeSocket() until the message is out
        socket = nil
        ws.send(.string(ClientMessage.pushToken(token: "", platform: "ios", env: settings.pushEnv).text)) { [weak self] error in
            ws.cancel(with: .goingAway, reason: nil)
            if error != nil { Task { @MainActor in self?.queueUnregister(server, token: token) } }
        }
    }

    func queueUnregister(_ server: String, token: String?) {
        if !settings.pushUnregister.contains(server) { settings.pushUnregister.append(server) }
        tokens.setToken(token ?? "", for: Self.unregisterKey(server))
        storage.save(settings)
        flushUnregisters()
    }

    /// Try the waiting unregisters (see stopPushes).
    func flushUnregisters() {
        for server in settings.pushUnregister where !unregistering.contains(server) {
            guard let url = Self.url(from: server) else {
                finishUnregister(server)
                continue
            }
            unregistering.insert(server)
            let token = tokens.token(for: Self.unregisterKey(server)).flatMap { $0.isEmpty ? nil : $0 }
            let hello = ClientMessage.hello(Hello(device: settings.device, token: token)).text
            let stop = ClientMessage.pushToken(token: "", platform: "ios", env: settings.pushEnv).text
            let session = self.session
            Task { [weak self] in
                let done = await Self.unregister(url: url, hello: hello, stop: stop, session: session)
                guard let self else { return }
                self.unregistering.remove(server)
                guard done else { return }
                self.finishUnregister(server)
                // signed in there again meanwhile: that sign-in's registration stands
                if self.state.isConnected, ServerAddress.same(self.settings.server, server), let ws = self.socket { self.sendPushToken(on: ws) }
            }
        }
    }

    func finishUnregister(_ server: String) {
        settings.pushUnregister.removeAll { $0 == server }
        tokens.setToken(nil, for: Self.unregisterKey(server))
        storage.save(settings)
    }

    /// One unregister connection: hello, wait for welcome, push_token "".
    /// True when it's done (or there's nothing left to do: the server
    /// doesn't know this device any more); false to try again later.
    nonisolated static func unregister(url: URL, hello: String, stop: String, session: URLSession) async -> Bool {
        let ws = session.webSocketTask(with: url)
        ws.maximumMessageSize = 64 * 1024 * 1024
        ws.resume()
        defer { ws.cancel(with: .goingAway, reason: nil) }
        do {
            try await ws.send(.string(hello))
            let deadline = Date().addingTimeInterval(20)
            while Date() < deadline {
                let text: String
                switch try await ws.receive() {
                case .string(let s): text = s
                case .data(let d): text = String(decoding: d, as: UTF8.self)
                @unknown default: continue
                }
                switch ServerMessage(text: text) {
                case .welcome:
                    try await ws.send(.string(stop))
                    // what the server sends after welcome (presence, mine...) is
                    // read and dropped meanwhile, or the answer to the ping waits behind it
                    let reader = Task { while !Task.isCancelled { _ = try await ws.receive() } }
                    defer { reader.cancel() }
                    // a ping answered means the server has read the message before it
                    try await withCheckedThrowingContinuation { (c: CheckedContinuation<Void, Error>) in
                        ws.sendPing { e in if let e { c.resume(throwing: e) } else { c.resume() } }
                    }
                    return true
                case let .error(code, _, _, _):
                    // signed out there for good (token unknown, access removed): the
                    // server dropped this device's notifications with it
                    return code == ServerErrorCode.auth.rawValue
                default:
                    continue
                }
            }
        } catch {}
        return false
    }
}
