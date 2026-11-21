// Whiteboards: pen, eraser, sticky notes (More tab).

import SwiftUI
import WorkboardKit

struct WhiteboardsView: View {
    @Environment(SyncClient.self) private var client

    var body: some View {
        let boards = client.data.map { client.view.whiteboards($0) } ?? []
        List {
            if boards.isEmpty {
                EmptyNote(title: "No whiteboards yet", systemImage: "scribble").listRowBackground(Color.clear)
            }
            ForEach(boards) { b in
                NavigationLink { WhiteboardCanvas(boardId: b.id) } label: {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(b.name).fontWeight(.medium)
                        Group {
                            if b.shapeCount > 0 {
                                Text("\(b.strokes.count) strokes · \(b.notes.count) notes · \(b.shapeCount) shapes")
                            } else {
                                Text("\(b.strokes.count) strokes · \(b.notes.count) notes")
                            }
                        }
                        .font(.caption).foregroundStyle(.secondary)
                    }
                }
                .deleteSwipe(client.canEdit) { client.edit { d, _ in Edits.deleteWhiteboard(&d, board: b.id) } }
                .accessibilityIdentifier("board-\(b.id)")
            }
        }
        .navigationTitle("Whiteboards")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                AddButton("New whiteboard", id: "newBoardButton") {
                    let id = client.newId()
                    client.edit { d, me in Edits.addWhiteboard(&d, me: me, id: id) }
                }
            }
        }
    }
}

struct WhiteboardCanvas: View {
    @Environment(SyncClient.self) private var client
    let boardId: UInt64
    enum Tool: String, CaseIterable { case pen = "Pen", eraser = "Eraser", note = "Sticky note", move = "Move" }
    @State private var tool: Tool = .pen
    @State private var color = WhiteboardView.colors[0]
    @State private var drawing: [[Double]] = []
    /// Where the board's top left corner is on screen.
    @State private var offset = CGSize.zero
    @State private var panStart: CGSize?
    @State private var dragging: (id: UInt64, start: [Double])?
    @State private var editing: StickyView?
    @State private var noteText = ""
    @State private var confirmClear = false

    var body: some View {
        let board = client.data.flatMap { client.view.whiteboards($0).first { $0.id == boardId } }
        VStack(spacing: 0) {
            toolbar
            GeometryReader { _ in
                ZStack(alignment: .topLeading) {
                    Color(.systemBackground)
                    Canvas { ctx, _ in
                        for s in board?.strokes ?? [] { draw(ctx, s.points, s.color, s.width) }
                        draw(ctx, drawing, color, 2.5)
                    }
                    ForEach(board?.notes ?? []) { n in
                        Text(n.text.isEmpty ? " " : n.text)
                            .font(.callout)
                            .padding(10)
                            .frame(width: WhiteboardView.noteSize.w, height: WhiteboardView.noteSize.h, alignment: .topLeading)
                            .background(RoundedRectangle(cornerRadius: 4).fill(Color(rgb: n.color).opacity(0.85)))
                            .offset(x: n.pos[0] + offset.width + (dragging?.id == n.id ? 0 : 0), y: n.pos[1] + offset.height)
                            .onTapGesture { if tool != .move { noteText = n.text; editing = n } }
                            .accessibilityIdentifier("note-\(n.id)")
                    }
                }
                .contentShape(Rectangle())
                .gesture(drag(board))
                .clipped()
            }
        }
        .navigationTitle(board?.name ?? "Whiteboard")
        .navigationBarTitleDisplayMode(.inline)
        .alert("Note", isPresented: Binding(isPresent: $editing)) {
            TextField("Text", text: $noteText)
            Button("Delete", role: .destructive) {
                if let n = editing { client.edit { d, _ in Edits.deleteNote(&d, board: boardId, note: n.id) } }
                editing = nil
            }
            Button("Save") {
                if let n = editing { client.edit { d, _ in Edits.updateNote(&d, board: boardId, note: n.id, text: noteText) } }
                editing = nil
            }
        }
        .confirmationDialog("Clear the whole board?", isPresented: $confirmClear, titleVisibility: .visible) {
            Button("Clear", role: .destructive) { client.edit { d, _ in Edits.clearWhiteboard(&d, board: boardId) } }
        }
    }

    var toolbar: some View {
        HStack(spacing: 12) {
            Picker("Tool", selection: $tool) {
                Image(systemName: "pencil.tip").tag(Tool.pen).accessibilityLabel("Pen")
                Image(systemName: "eraser").tag(Tool.eraser).accessibilityLabel("Eraser")
                Image(systemName: "note.text").tag(Tool.note).accessibilityLabel("Sticky note")
                Image(systemName: "hand.draw").tag(Tool.move).accessibilityLabel("Move")
            }
            .pickerStyle(.segmented)
            .frame(maxWidth: 220)
            .accessibilityIdentifier("boardTool")
            ForEach(WhiteboardView.colors, id: \.self) { c in
                Circle().fill(Color(rgb: c)).frame(width: 20, height: 20)
                    .overlay(Circle().stroke(Color.primary, lineWidth: c == color ? 2 : 0).padding(-3))
                    .onTapGesture { color = c }
                    .accessibilityAddTraits(.isButton)
            }
            Spacer(minLength: 0)
            Button { confirmClear = true } label: { Image(systemName: "trash") }
                .disabled(!client.canEdit)
                .accessibilityLabel("Clear board")
        }
        .padding(.horizontal)
        .padding(.vertical, 8)
        .background(.bar)
    }

    func draw(_ ctx: GraphicsContext, _ pts: [[Double]], _ c: [Int], _ w: Double) {
        guard let first = pts.first else { return }
        var p = Path()
        p.move(to: CGPoint(x: first[0] + offset.width, y: first[1] + offset.height))
        if pts.count == 1 { p.addLine(to: CGPoint(x: first[0] + offset.width + 0.1, y: first[1] + offset.height)) }
        for q in pts.dropFirst() { p.addLine(to: CGPoint(x: q[0] + offset.width, y: q[1] + offset.height)) }
        ctx.stroke(p, with: .color(Color(rgb: c)), style: StrokeStyle(lineWidth: w, lineCap: .round, lineJoin: .round))
    }

    /// Board coordinates of a point on screen.
    func board(_ p: CGPoint) -> [Double] { [Double(p.x - offset.width), Double(p.y - offset.height)] }

    func drag(_ b: WhiteboardView?) -> some Gesture {
        DragGesture(minimumDistance: 0)
            .onChanged { g in
                guard client.canEdit || tool == .move else { return }
                switch tool {
                case .pen: drawing.append(board(g.location))
                case .move:
                    // a note under the finger moves; otherwise the board does
                    let start = board(g.startLocation)
                    if dragging == nil, panStart == nil {
                        if let n = b?.notes.last(where: { start[0] >= $0.pos[0] && start[0] <= $0.pos[0] + WhiteboardView.noteSize.w && start[1] >= $0.pos[1] && start[1] <= $0.pos[1] + WhiteboardView.noteSize.h }) {
                            dragging = (n.id, n.pos)
                        } else {
                            panStart = offset
                        }
                    }
                    if let p = panStart {
                        offset = CGSize(width: p.width + g.translation.width, height: p.height + g.translation.height)
                    }
                default: break
                }
            }
            .onEnded { g in
                switch tool {
                case .pen:
                    let pts = drawing
                    drawing = []
                    client.edit { d, _ in Edits.addStroke(&d, board: boardId, points: pts, color: color, width: 2.5) }
                case .eraser:
                    let p = board(g.location)
                    // the stroke passing nearest the finger
                    if let s = b?.strokes.min(by: { dist($0, p) < dist($1, p) }), dist(s, p) < 24 {
                        client.edit { d, _ in Edits.eraseStroke(&d, board: boardId, stroke: s.id) }
                    }
                case .note:
                    let p = board(g.location)
                    client.edit { d, _ in Edits.addNote(&d, board: boardId, at: p, color: color) }
                case .move:
                    if let (id, start) = dragging {
                        let pos = [start[0] + g.translation.width, start[1] + g.translation.height]
                        client.edit { d, _ in Edits.updateNote(&d, board: boardId, note: id, pos: pos) }
                    }
                    dragging = nil
                    panStart = nil
                }
            }
    }

    func dist(_ s: StrokeView, _ p: [Double]) -> Double {
        s.points.map { hypot($0[0] - p[0], $0[1] - p[1]) }.min() ?? .infinity
    }
}
