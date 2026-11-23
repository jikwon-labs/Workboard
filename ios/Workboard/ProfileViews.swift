// Edit profile and Set status, as on the Mac (src/app/profile.rs and the
// profile menu): my photo, full name, display name and title, and a few words
// on what I'm up to. Avatars everywhere show a member's photo over their
// initials once it's here.

import PhotosUI
import SwiftUI
import WorkboardKit

// MARK: photos in avatars

/// Profile photos come down once (kept with the other files) and are decoded
/// at the size they're shown. One download per photo however many avatars
/// show it; one that couldn't come down isn't asked for again for a minute.
@MainActor
enum AvatarPhotos {
    private static var loading: [String: Task<URL?, Never>] = [:]
    private static var failed: [String: Date] = [:]

    /// The photo `file` (named `name`) decoded at `pixels`, nil while it
    /// isn't here.
    static func image(file: String, name: String, pixels: Int, client: SyncClient) async -> UIImage? {
        guard let url = await local(file: file, name: name, client: client) else { return nil }
        return await PictureCache.thumbnail(url, key: file, maxPixels: pixels)
    }

    /// Already decoded at `pixels`: drawn at once, without initials first.
    static func cached(file: String, pixels: Int) -> UIImage? {
        PictureCache.cached(key: file, maxPixels: pixels)
    }

    /// This iPhone's copy of the photo, downloading it first when needed.
    static func local(file: String, name: String, client: SyncClient) async -> URL? {
        let ext = (name as NSString).pathExtension.lowercased()
        var dest = FileAPI.cached(file)
        if !ext.isEmpty { dest.appendPathExtension(ext) }
        if FileManager.default.fileExists(atPath: dest.path) { return dest }
        if let t = loading[file] { return await t.value }
        if let at = failed[file], Date().timeIntervalSince(at) < 60 { return nil }
        let (server, auth) = (client.settings.server, client.fileAuth)
        let t = Task { try? await FileAPI.download(server: server, auth: auth, sha: file, ext: ext) }
        loading[file] = t
        let url = await t.value
        loading[file] = nil
        failed[file] = url == nil ? Date() : nil
        return url
    }
}

/// A member's photo filling a circle of `side`, once it's here (until then
/// it's clear and the initials under it show).
struct AvatarPhoto: View {
    @Environment(SyncClient.self) private var client: SyncClient?
    @Environment(\.displayScale) private var scale
    let file: String
    let name: String
    let side: CGFloat
    @State private var image: UIImage?

    var pixels: Int { max(1, Int((side * scale).rounded(.up))) }

    var body: some View {
        // the photo's own decode first: `image` may still be the one before
        let shown = AvatarPhotos.cached(file: file, pixels: pixels) ?? image
        Color.clear
            .frame(width: side, height: side)
            .overlay {
                if let shown { Image(uiImage: shown).resizable().scaledToFill() }
            }
            .clipShape(Circle())
            .task(id: file) {
                guard let client else { return }
                image = await AvatarPhotos.image(file: file, name: name, pixels: pixels, client: client)
            }
    }
}

// MARK: Edit profile

/// Edit profile (the Mac's): my photo, full name, display name and title. A
/// photo chosen here is cut to its middle square, shrunk to 512 pixels and
/// put on the team server at once; Save writes what changed.
struct ProfileEditView: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    @Environment(\.displayScale) private var scale
    @State private var name = ""
    @State private var display = ""
    @State private var title = ""
    /// The photo Save writes (an Attachment), nil for none.
    @State private var photo: JSON?
    @State private var filled = false
    @State private var picked: PhotosPickerItem?
    @State private var uploading = false
    @State private var problem: String?
    /// The photo as shown here: the one just chosen, or the saved one once
    /// it's down.
    @State private var shown: UIImage?

    static let side: CGFloat = 96

    var me: UInt64? { client.me }
    var member: MemberView? { me.flatMap { client.view.member($0) } }
    var photoFile: String? { photo?["file"]?.string }

    var body: some View {
        Form {
            Section {
                VStack(spacing: 12) {
                    preview
                    // plain words in one line under the photo, as in Contacts
                    // (framed buttons of two widths, one broken onto two
                    // lines, looked off)
                    HStack(spacing: 28) {
                        PhotosPicker(selection: $picked, matching: .images) {
                            Text("Upload photo")
                        }
                        .accessibilityIdentifier("uploadPhotoButton")
                        if photo != nil {
                            Button("Remove photo", role: .destructive) {
                                photo = nil
                                shown = nil
                                problem = nil
                            }
                            .accessibilityIdentifier("removePhotoButton")
                        }
                    }
                    .lineLimit(1)
                    .buttonStyle(.borderless)
                    .disabled(uploading || !client.canEdit)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 6)
                if let problem {
                    Text(L(problem)).font(.footnote).foregroundStyle(.red)
                        .accessibilityIdentifier("photoProblem")
                }
            } header: {
                Text("Profile photo")
            }
            Section {
                TextField("Full name", text: $name)
                    .textContentType(.name)
                    .accessibilityIdentifier("profileNameField")
            } header: {
                Text("Full name")
            }
            Section {
                TextField("Display name", text: $display)
                    .accessibilityIdentifier("profileDisplayNameField")
            } header: {
                Text("Display name")
            } footer: {
                Text("This could be your first name or a nickname: what you'd like people to call you. Others see it before your full name.")
            }
            Section {
                TextField("Job title", text: $title)
                    .textContentType(.jobTitle)
                    .accessibilityIdentifier("profileTitleField")
            } header: {
                Text("Job title")
            } footer: {
                Text("Let people know what you do at \(client.view.name).")
            }
        }
        .navigationTitle("Edit profile")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("Save", action: save)
                    .disabled(!client.canEdit || uploading || name.trimmed.isEmpty)
                    .accessibilityIdentifier("saveProfileButton")
            }
        }
        .onAppear(perform: fill)
        .onChange(of: picked) { _, item in
            if let item { Task { await use(item) } }
        }
        .task(id: photoFile) { await loadShown() }
    }

    /// The photo, or the initials of the name as typed on my colour.
    var preview: some View {
        let typed = display.trimmed.isEmpty ? name.trimmed : display.trimmed
        return ZStack {
            Circle().fill(member.map { Color(rgb: $0.color) } ?? .gray)
            Text(MemberView.initials(of: typed))
                .font(.system(size: Self.side * 0.36, weight: .semibold))
                .foregroundStyle(.white)
            if let shown { Image(uiImage: shown).resizable().scaledToFill() }
            if uploading {
                Circle().fill(.black.opacity(0.35))
                ProgressView().tint(.white)
            }
        }
        .frame(width: Self.side, height: Self.side)
        .clipShape(Circle())
        .accessibilityElement()
        .accessibilityLabel(Text("Profile photo"))
        .accessibilityValue(shown != nil ? Text("Photo") : Text("No photo"))
        .accessibilityIdentifier("profilePhoto")
    }

    func fill() {
        guard !filled, let m = member else { return }
        filled = true
        name = m.fullName
        display = m.displayName
        title = m.title
        // kept as the member has it, so saving without a new one writes nothing
        let raw = client.data?["members"]?.array?.first { $0["id"]?.uint == m.id }
        photo = m.photo == nil ? nil : raw?["photo"]
    }

    func loadShown() async {
        guard let file = photoFile else {
            shown = nil
            return
        }
        let img = await AvatarPhotos.image(file: file, name: photo?["name"]?.string ?? "", pixels: Int(Self.side * scale), client: client)
        if photoFile == file, let img { shown = img }
    }

    /// The picked picture as my photo.
    func use(_ item: PhotosPickerItem) async {
        picked = nil
        problem = nil
        uploading = true
        defer { uploading = false }
        guard let raw = try? await item.loadTransferable(type: Data.self) else {
            problem = "That picture couldn't be read."
            return
        }
        await use(raw)
    }

    /// A picture as my photo: made square and small here, then put on the
    /// server (Save names it).
    func use(_ raw: Data) async {
        uploading = true
        defer { uploading = false }
        guard let made = await Task.detached(priority: .userInitiated, operation: { PictureConvert.profilePhoto(raw) }).value else {
            problem = "That picture couldn't be read."
            return
        }
        do {
            let sha = try await FileAPI.upload(server: client.settings.server, auth: client.fileAuth, data: made.png, contentType: "image/png")
            FileAPI.keep(made.png, sha: sha, ext: "png")
            shown = UIImage(data: made.png)
            photo = FileAPI.attachment(sha: sha, name: Edits.profilePhotoName, isVideo: false, bytes: made.png.count, w: made.side, h: made.side, secs: 0)
        } catch let e as FileError {
            problem = e.explanation
        } catch {
            problem = error.localizedDescription
        }
    }

    func save() {
        guard let me else { return }
        client.edit { d, _ in Edits.setProfile(&d, member: me, name: name, displayName: display, title: title, photo: photo) }
        dismiss()
    }
}

// MARK: Set status

/// Set status (the Mac's): a few words on what I'm up to, shown with my name.
/// A suggestion fills in the words, emoji and all, as on the Mac.
struct StatusEditView: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    @State private var text = ""
    @State private var filled = false

    /// The Mac's suggestions in the phone's language; the emoji stays part
    /// of the status people see.
    static var suggestions: [String] {
        [
            String(localized: "\u{1F4C5} In a meeting"),
            String(localized: "\u{1F68C} Commuting"),
            String(localized: "\u{2639} Out sick"),
            String(localized: "\u{1F334} Vacationing"),
            String(localized: "\u{1F3E0} Working remotely"),
        ]
    }

    var current: String { client.me.flatMap { client.view.member($0)?.statusText } ?? "" }

    var body: some View {
        Form {
            Section {
                TextField("What's your status?", text: $text)
                    .submitLabel(.done)
                    .onSubmit(save)
                    .accessibilityIdentifier("statusField")
            }
            Section {
                ForEach(Array(Self.suggestions.enumerated()), id: \.offset) { i, s in
                    Button {
                        text = s
                    } label: {
                        // a choice, not an action: plain text (`.primary` here
                        // would be the button's tint), ticked when chosen
                        HStack {
                            Text(verbatim: s).foregroundStyle(Color.primary)
                            Spacer()
                            if text.trimmed == s {
                                Image(systemName: "checkmark").foregroundStyle(Color.wbAccent)
                            }
                        }
                    }
                    .accessibilityIdentifier("statusSuggestion-\(i)")
                }
            }
            if !current.isEmpty {
                Section {
                    Button("Clear status", role: .destructive) { set("") }
                        .accessibilityIdentifier("clearStatusButton")
                }
            }
        }
        .disabled(!client.canEdit)
        .navigationTitle("Set status")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("Save", action: save)
                    .disabled(!client.canEdit)
                    .accessibilityIdentifier("saveStatusButton")
            }
        }
        .onAppear {
            if !filled {
                filled = true
                text = current
            }
        }
    }

    func save() { set(text) }

    func set(_ status: String) {
        guard let me = client.me else { return }
        client.edit { d, _ in Edits.setStatusText(&d, member: me, to: status) }
        dismiss()
    }
}
