// Whiteboards (Whiteboard, WbStroke and Sticky in src/model.rs; the canvas in
// src/app/more.rs): pen strokes and sticky notes in board coordinates.
// Strokes and notes are written without an id; SyncClient.edit gives them
// theirs (Sync.fillBoardIds) the way the Mac app does before it saves.

import Foundation

public struct StrokeView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    public let color: [Int]
    public let width: Double
    public let points: [[Double]]

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        color = (j["color"]?.array ?? []).compactMap { $0.int.map(Int.init) }
        width = j["width"]?.double ?? 2.5
        points = (j["points"]?.array ?? []).map { ($0.array ?? []).compactMap(\.double) }.filter { $0.count == 2 }
    }
}

public struct StickyView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    public let pos: [Double]
    public let text: String
    public let color: [Int]

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        let p = (j["pos"]?.array ?? []).compactMap(\.double)
        pos = p.count == 2 ? p : [0, 0]
        text = j["text"]?.string ?? ""
        color = (j["color"]?.array ?? []).compactMap { $0.int.map(Int.init) }
    }
}

public struct WhiteboardView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    public let name: String
    public let strokes: [StrokeView]
    public let notes: [StickyView]
    public let owner: UInt64

    /// WB_COLORS in src/app/more.rs.
    public static let colors: [[Int]] = [[32, 32, 32], [0xd3, 0x3d, 0x44], [8, 128, 234], [41, 151, 100], [0xf8, 0xae, 0x00], [0x5f, 0x55, 0xee]]
    /// A sticky note's size in board points.
    public static let noteSize = (w: 160.0, h: 120.0)

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        name = j["name"]?.string ?? ""
        strokes = (j["strokes"]?.array ?? []).map(StrokeView.init)
        notes = (j["notes"]?.array ?? []).map(StickyView.init)
        owner = j["owner"]?.uint ?? 0
    }
}

extension WorkspaceView {
    public func whiteboards(_ data: JSON) -> [WhiteboardView] { (data.at("extra.whiteboards")?.array ?? []).map(WhiteboardView.init) }
}

extension Edits {
    @discardableResult
    public static func addWhiteboard(_ data: inout JSON, me: UInt64, id: UInt64) -> Bool {
        var bs = data.at("extra.whiteboards")?.array ?? []
        bs.append(["id": .id(id), "name": .string("Whiteboard \(bs.count + 1)"), "strokes": [], "notes": [], "owner": .id(me)])
        data.set("extra.whiteboards", .array(bs))
        return true
    }

    private static func changeBoard(_ data: inout JSON, _ id: UInt64, _ change: (inout JSON) -> Bool) -> Bool {
        var bs = data.at("extra.whiteboards")?.array ?? []
        guard let i = bs.firstIndex(where: { $0["id"]?.uint == id }), change(&bs[i]) else { return false }
        data.set("extra.whiteboards", .array(bs))
        return true
    }

    @discardableResult
    public static func renameWhiteboard(_ data: inout JSON, board: UInt64, to name: String) -> Bool {
        let clean = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !clean.isEmpty else { return false }
        return changeBoard(&data, board) { $0["name"] = .string(clean); return true }
    }

    @discardableResult
    public static func deleteWhiteboard(_ data: inout JSON, board: UInt64) -> Bool {
        var bs = data.at("extra.whiteboards")?.array ?? []
        let n = bs.count
        bs.removeAll { $0["id"]?.uint == board }
        guard bs.count != n else { return false }
        data.set("extra.whiteboards", .array(bs))
        return true
    }

    /// A pen stroke; a single point is kept as a dot.
    @discardableResult
    public static func addStroke(_ data: inout JSON, board: UInt64, points: [[Double]], color: [Int], width: Double) -> Bool {
        guard !points.isEmpty else { return false }
        return changeBoard(&data, board) { b in
            b.append(at: "strokes", ["color": .array(color.map { .int(Int64($0)) }), "width": .double(width), "points": .array(points.map { .array($0.map(JSON.double)) })])
            return true
        }
    }

    /// The eraser takes whole strokes.
    @discardableResult
    public static func eraseStroke(_ data: inout JSON, board: UInt64, stroke: UInt64) -> Bool {
        changeBoard(&data, board) { b in
            var s = b["strokes"]?.array ?? []
            let n = s.count
            s.removeAll { $0["id"]?.uint == stroke }
            b["strokes"] = .array(s)
            return s.count != n
        }
    }

    /// A sticky note centred on `at` (yellow when the pen is black).
    @discardableResult
    public static func addNote(_ data: inout JSON, board: UInt64, at: [Double], color: [Int]) -> Bool {
        let c = color == [32, 32, 32] ? [0xf8, 0xae, 0x00] : color
        return changeBoard(&data, board) { b in
            b.append(at: "notes", ["pos": [.double(at[0] - WhiteboardView.noteSize.w / 2), .double(at[1] - WhiteboardView.noteSize.h / 2)], "text": "", "color": .array(c.map { .int(Int64($0)) })])
            return true
        }
    }

    @discardableResult
    public static func updateNote(_ data: inout JSON, board: UInt64, note: UInt64, text: String? = nil, pos: [Double]? = nil) -> Bool {
        changeBoard(&data, board) { b in
            b.updateItem(at: "notes", id: note) { n in
                if let text { n["text"] = .string(text) }
                if let pos { n["pos"] = .array(pos.map(JSON.double)) }
            }
        }
    }

    @discardableResult
    public static func deleteNote(_ data: inout JSON, board: UInt64, note: UInt64) -> Bool {
        changeBoard(&data, board) { b in
            var ns = b["notes"]?.array ?? []
            let n = ns.count
            ns.removeAll { $0["id"]?.uint == note }
            b["notes"] = .array(ns)
            return ns.count != n
        }
    }

    @discardableResult
    public static func clearWhiteboard(_ data: inout JSON, board: UInt64) -> Bool {
        changeBoard(&data, board) { b in
            b["strokes"] = []
            b["notes"] = []
            return true
        }
    }
}
