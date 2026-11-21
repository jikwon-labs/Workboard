// Posts and saved messages (More tab).

import SwiftUI
import WorkboardKit

struct MessageListView: View {
    @Environment(SyncClient.self) private var client
    let title: String
    let posts: Bool

    var body: some View {
        let v = client.view
        let me = client.me ?? 0
        let rows = posts ? v.posts(me: me) : (client.data.map { v.savedMessages($0, me: me) } ?? [])
        List {
            if rows.isEmpty {
                EmptyNote(title: posts ? "There are no Posts yet!" : "Nothing saved", systemImage: posts ? "megaphone" : "bookmark",
                          detail: posts ? "Write one from a channel's megaphone button." : "Long-press a message and choose Save for later.")
                    .listRowBackground(Color.clear)
            }
            ForEach(rows, id: \.message.id) { c, m in
                NavigationLink(value: Nav.Route.channel(c.id)) {
                    VStack(alignment: .leading, spacing: 3) {
                        Text("\(v.member(m.author)?.name ?? String(localized: "Someone")) in \(v.place(of: c, me: me)) · \(Fmt.time(m.ts))")
                            .font(.caption).foregroundStyle(.secondary)
                        if let t = shownPostTitle(m) { Text(verbatim: t).font(.headline) }
                        if !m.text.isEmpty { Text(m.text).lineLimit(4) }
                    }
                }
                .accessibilityIdentifier("listed-\(m.id)")
            }
        }
        .navigationTitle(title)
    }
}
