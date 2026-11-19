// Workspaces kept on this iPhone, to switch between like the Mac app's
// Switch Workspace menu.
//
// The workspace open now lives in the storage folder itself (settings.json,
// snapshot.json, trash.json). Leaving it for another server moves those
// files to workspaces/<key>/ and lists it in workspaces.json, so the switch
// list never has to read a whole copy; going back moves them home again.
// The device token for each server stays in the token store (one per
// address), so a kept workspace opens signed in, and edits not yet on its
// server wait with it until it is open again.
//
// Every kept workspace keeps getting push notifications (Push.swift):
// leaving one doesn't tell its server to stop, removing it does.

import Foundation

/// A workspace kept on this iPhone while another one is open.
public struct SavedWorkspace: Codable, Equatable, Sendable, Identifiable {
    /// The server address it was opened with.
    public var server: String
    /// The workspace's name ("" when its copy never came).
    public var name: String
    /// Who was signed in or picked there (nil: nobody yet).
    public var who: String?
    /// Edits made there that its server hasn't got yet.
    public var waiting: Int
    /// The folder below workspaces/ that holds it.
    public var folder: String

    public var id: String { folder }

    public init(server: String, name: String, who: String? = nil, waiting: Int = 0, folder: String) {
        self.server = server
        self.name = name
        self.who = who
        self.waiting = waiting
        self.folder = folder
    }
}

extension ServerAddress {
    /// The folder name a workspace on this address is kept under: the same
    /// for every way of typing the same server.
    static func folderKey(_ address: String) -> String {
        let a = websocket(address)?.absoluteString.lowercased() ?? address.trimmed
        return String(FileAPI.sha256(Data(a.utf8)).prefix(20))
    }
}

extension DeviceSettings {
    /// These settings for another workspace on this same iPhone: what
    /// belongs to the device (its id, its push token and the servers still
    /// to be told to stop pushes) comes from `device`.
    func onDevice(_ device: DeviceSettings) -> DeviceSettings {
        var s = self
        s.device = device.device
        s.pushToken = device.pushToken
        s.pushEnv = device.pushEnv
        s.pushSentTo = device.pushSentTo
        s.pushUnregister = device.pushUnregister
        return s
    }
}

extension Storage {
    var savedWorkspacesURL: URL { folder.appendingPathComponent("workspaces.json") }

    /// Where a kept workspace's files are.
    func kept(_ key: String) -> Storage {
        Storage(folder: folder.appendingPathComponent("workspaces", isDirectory: true).appendingPathComponent(key, isDirectory: true))
    }

    public func loadSaved() -> [SavedWorkspace] {
        guard let d = try? Data(contentsOf: savedWorkspacesURL), let list = try? JSONDecoder().decode([SavedWorkspace].self, from: d) else { return [] }
        // a folder removed by hand (or never finished): not listed
        return list.filter { FileManager.default.fileExists(atPath: kept($0.folder).settingsURL.path) }
    }

    func save(saved: [SavedWorkspace]) {
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        if let d = try? JSONEncoder().encode(saved) { try? d.write(to: savedWorkspacesURL, options: .atomic) }
    }

    /// Move one file from here to `to` (replacing what is there); a file
    /// that isn't here leaves none there.
    func move(_ from: URL, to: URL) {
        let fm = FileManager.default
        try? fm.removeItem(at: to)
        guard fm.fileExists(atPath: from.path) else { return }
        try? fm.createDirectory(at: to.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? fm.moveItem(at: from, to: to)
    }
}

extension SyncClient {
    /// Open a kept workspace in place of this one (which is kept in turn).
    public func switchTo(_ saved: SavedWorkspace) {
        setServer(saved.server, freshCopy: true)
    }

    /// Remove a kept workspace from this iPhone: its copy, its edits still
    /// waiting and its sign-in. Nothing changes on its server.
    public func forget(_ saved: SavedWorkspace) {
        // its server stops sending this iPhone notifications
        if settings.pushToken != nil { queueUnregister(saved.server, token: tokens.token(for: saved.server)) }
        try? FileManager.default.removeItem(at: storage.kept(saved.folder).folder)
        tokens.setToken(nil, for: saved.server)
        savedWorkspaces.removeAll { $0.folder == saved.folder }
        storage.save(saved: savedWorkspaces)
    }

    /// Put the open workspace aside before leaving it for another server
    /// (see setServer). Nothing to keep when its copy never came and it
    /// wasn't signed in. False when nothing was kept.
    @discardableResult
    func keepForLater() -> Bool {
        guard !settings.server.isEmpty, data != nil || hasToken else { return false }
        let key = ServerAddress.folderKey(settings.server)
        let place = storage.kept(key)
        flushSnapshot()
        storage.move(storage.snapshotURL, to: place.snapshotURL)
        place.save(trash: trash)
        place.save(settings)
        let who = settings.me.flatMap { view.member($0)?.name }
        let entry = SavedWorkspace(server: settings.server, name: view.name, who: who, waiting: waitingCount, folder: key)
        savedWorkspaces.removeAll { $0.folder == key }
        savedWorkspaces.insert(entry, at: 0)
        storage.save(saved: savedWorkspaces)
        return true
    }

    /// Open the workspace kept for `address`, if there is one: its copy,
    /// Trash and settings come back (settings.server stays as typed now).
    /// False when nothing is kept for it.
    func reopenKept(_ address: String) -> Bool {
        let key = ServerAddress.folderKey(address)
        let place = storage.kept(key)
        guard FileManager.default.fileExists(atPath: place.settingsURL.path) else { return false }
        var kept = place.loadSettings().onDevice(settings)
        kept.server = settings.server
        storage.move(place.snapshotURL, to: storage.snapshotURL)
        let snap = storage.loadSnapshot().map(Self.prepared)
        data = snap
        view = snap.map { WorkspaceView($0) } ?? .empty
        rev = 0
        settings = kept
        trash = place.loadTrash()
        storage.save(trash: trash)
        try? FileManager.default.removeItem(at: place.folder)
        savedWorkspaces.removeAll { $0.folder == key }
        storage.save(saved: savedWorkspaces)
        return true
    }
}
