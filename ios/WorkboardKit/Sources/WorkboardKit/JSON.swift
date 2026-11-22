// A raw JSON tree. The workspace snapshot is kept in this form end to end so
// that fields the iOS app does not know about (Mac-only fields, newer fields)
// survive a round trip through the phone and are never dropped on the server.
//
// Numbers keep their kind: integers stay integers (ids are ms * 1024 + tag and
// fingerprints are full u64 values), floats stay floats.

import Foundation

public enum JSON: Equatable, Sendable {
    case null
    case bool(Bool)
    case int(Int64)
    /// Only for integers above Int64.max (fingerprints in extra.sync.hashes).
    case uint(UInt64)
    case double(Double)
    case string(String)
    case array([JSON])
    case object([String: JSON])

    // MARK: reading

    public subscript(key: String) -> JSON? {
        get { if case .object(let o) = self { return o[key] } else { return nil } }
        set {
            guard case .object(var o) = self else { return }
            // let go of self's hold on the object first, so it changes in
            // place instead of being copied whole
            self = .null
            o[key] = newValue
            self = .object(o)
        }
    }

    public subscript(index: Int) -> JSON? {
        if case .array(let a) = self, a.indices.contains(index) { return a[index] }
        return nil
    }

    public var object: [String: JSON]? { if case .object(let o) = self { return o } else { return nil } }
    public var array: [JSON]? { if case .array(let a) = self { return a } else { return nil } }
    public var string: String? { if case .string(let s) = self { return s } else { return nil } }
    public var bool: Bool? { if case .bool(let b) = self { return b } else { return nil } }
    public var isNull: Bool { self == .null }

    public var int: Int64? {
        switch self {
        case .int(let i): return i
        case .uint(let u): return u <= UInt64(Int64.max) ? Int64(u) : nil
        default: return nil
        }
    }

    /// Like serde_json's `as_u64`: non-negative integers only.
    public var uint: UInt64? {
        switch self {
        case .int(let i): return i >= 0 ? UInt64(i) : nil
        case .uint(let u): return u
        default: return nil
        }
    }

    public var double: Double? {
        switch self {
        case .int(let i): return Double(i)
        case .uint(let u): return Double(u)
        case .double(let d): return d
        default: return nil
        }
    }

    public static func id(_ v: UInt64) -> JSON { v <= UInt64(Int64.max) ? .int(Int64(v)) : .uint(v) }

    // MARK: paths ("extra.sync")

    public func at(_ path: String) -> JSON? {
        var cur: JSON? = self
        for part in path.split(separator: ".") { cur = cur?[String(part)] }
        return cur
    }

    /// Sets a value at a dotted path, creating objects on the way (like set_path).
    public mutating func set(_ path: String, _ value: JSON) {
        let parts = path.split(separator: ".").map(String.init)
        setParts(parts[...], value)
    }

    private mutating func setParts(_ parts: ArraySlice<String>, _ value: JSON) {
        guard let first = parts.first else { self = value; return }
        guard case .object(var o) = self else {
            self = .object([:])
            setParts(parts, value)
            return
        }
        // taken out while it changes, so nothing is copied on the way down
        self = .null
        if parts.count == 1 {
            o[first] = value
        } else {
            var child = o.removeValue(forKey: first) ?? .object([:])
            child.setParts(parts.dropFirst(), value)
            o[first] = child
        }
        self = .object(o)
    }

    /// Change this object in place (nothing happens to anything else). The
    /// object is taken out of `self` while it changes, so it isn't copied.
    @discardableResult
    public mutating func withObject<R>(_ change: (inout [String: JSON]) -> R) -> R? {
        guard case .object(var o) = self else { return nil }
        self = .null
        let r = change(&o)
        self = .object(o)
        return r
    }
}

// MARK: - parsing

public struct JSONParseError: Error, CustomStringConvertible {
    public let offset: Int
    public let reason: String
    public var description: String { "JSON parse error at \(offset): \(reason)" }
}

extension JSON {
    public init(parsing data: Data) throws {
        var p = Parser(bytes: [UInt8](data))
        p.skipWS()
        self = try p.value()
        p.skipWS()
        if p.i != p.bytes.count { throw JSONParseError(offset: p.i, reason: "trailing characters") }
    }

    public init(parsing text: String) throws {
        try self.init(parsing: Data(text.utf8))
    }
}

private struct Parser {
    let bytes: [UInt8]
    var i = 0

    func fail(_ r: String) -> JSONParseError { JSONParseError(offset: i, reason: r) }

    mutating func skipWS() {
        while i < bytes.count, [0x20, 0x0A, 0x0D, 0x09].contains(bytes[i]) { i += 1 }
    }

    mutating func expect(_ word: String) throws {
        for b in word.utf8 {
            guard i < bytes.count, bytes[i] == b else { throw fail("expected \(word)") }
            i += 1
        }
    }

    mutating func value() throws -> JSON {
        guard i < bytes.count else { throw fail("unexpected end") }
        switch bytes[i] {
        case UInt8(ascii: "{"):
            i += 1
            var o: [String: JSON] = [:]
            skipWS()
            if i < bytes.count, bytes[i] == UInt8(ascii: "}") { i += 1; return .object(o) }
            while true {
                skipWS()
                guard i < bytes.count, bytes[i] == UInt8(ascii: "\"") else { throw fail("expected key") }
                let k = try string()
                skipWS()
                try expect(":")
                skipWS()
                o[k] = try value()
                skipWS()
                guard i < bytes.count else { throw fail("unexpected end") }
                if bytes[i] == UInt8(ascii: ",") { i += 1; continue }
                if bytes[i] == UInt8(ascii: "}") { i += 1; return .object(o) }
                throw fail("expected , or }")
            }
        case UInt8(ascii: "["):
            i += 1
            var a: [JSON] = []
            skipWS()
            if i < bytes.count, bytes[i] == UInt8(ascii: "]") { i += 1; return .array(a) }
            while true {
                skipWS()
                a.append(try value())
                skipWS()
                guard i < bytes.count else { throw fail("unexpected end") }
                if bytes[i] == UInt8(ascii: ",") { i += 1; continue }
                if bytes[i] == UInt8(ascii: "]") { i += 1; return .array(a) }
                throw fail("expected , or ]")
            }
        case UInt8(ascii: "\""):
            return .string(try string())
        case UInt8(ascii: "t"):
            try expect("true"); return .bool(true)
        case UInt8(ascii: "f"):
            try expect("false"); return .bool(false)
        case UInt8(ascii: "n"):
            try expect("null"); return .null
        default:
            return try number()
        }
    }

    mutating func number() throws -> JSON {
        let start = i
        var isFloat = false
        if i < bytes.count, bytes[i] == UInt8(ascii: "-") { i += 1 }
        while i < bytes.count {
            let b = bytes[i]
            if b >= 0x30 && b <= 0x39 { i += 1; continue }
            if b == UInt8(ascii: ".") || b == UInt8(ascii: "e") || b == UInt8(ascii: "E") || b == UInt8(ascii: "+") || (b == UInt8(ascii: "-") && i > start) {
                isFloat = true; i += 1; continue
            }
            break
        }
        guard i > start, let text = String(bytes: bytes[start..<i], encoding: .utf8) else { throw fail("bad number") }
        if !isFloat {
            if let v = Int64(text) { return .int(v) }
            if let v = UInt64(text) { return .uint(v) }
        }
        guard let d = Double(text) else { throw fail("bad number") }
        return .double(d)
    }

    mutating func hex4() throws -> UInt32 {
        guard i + 4 <= bytes.count, let s = String(bytes: bytes[i..<i + 4], encoding: .ascii), let v = UInt32(s, radix: 16) else { throw fail("bad \\u escape") }
        i += 4
        return v
    }

    mutating func string() throws -> String {
        i += 1 // opening quote
        var out = [UInt8]()
        while true {
            guard i < bytes.count else { throw fail("unterminated string") }
            let b = bytes[i]
            i += 1
            if b == UInt8(ascii: "\"") { break }
            if b != UInt8(ascii: "\\") { out.append(b); continue }
            guard i < bytes.count else { throw fail("bad escape") }
            let e = bytes[i]
            i += 1
            switch e {
            case UInt8(ascii: "\""): out.append(0x22)
            case UInt8(ascii: "\\"): out.append(0x5C)
            case UInt8(ascii: "/"): out.append(0x2F)
            case UInt8(ascii: "b"): out.append(0x08)
            case UInt8(ascii: "f"): out.append(0x0C)
            case UInt8(ascii: "n"): out.append(0x0A)
            case UInt8(ascii: "r"): out.append(0x0D)
            case UInt8(ascii: "t"): out.append(0x09)
            case UInt8(ascii: "u"):
                var cp = try hex4()
                if cp >= 0xD800 && cp < 0xDC00, i + 1 < bytes.count, bytes[i] == UInt8(ascii: "\\"), bytes[i + 1] == UInt8(ascii: "u") {
                    i += 2
                    let lo = try hex4()
                    cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00)
                }
                let scalar = Unicode.Scalar(cp) ?? "\u{FFFD}"
                out.append(contentsOf: Array(String(Character(scalar)).utf8))
            default: throw fail("bad escape")
            }
        }
        return String(decoding: out, as: UTF8.self)
    }
}

// MARK: - writing

extension JSON {
    /// Compact JSON with object keys sorted, so the same tree always gives the
    /// same text (fingerprints depend on this).
    public func serialized() -> String {
        var out = ""
        write(into: &out)
        return out
    }

    public func data() -> Data { Data(serialized().utf8) }

    private func write(into out: inout String) {
        switch self {
        case .null: out += "null"
        case .bool(let b): out += b ? "true" : "false"
        case .int(let i): out += String(i)
        case .uint(let u): out += String(u)
        case .double(let d):
            if d.isFinite { out += "\(d)" } else { out += "null" }
        case .string(let s): JSON.writeString(s, into: &out)
        case .array(let a):
            out += "["
            for (n, v) in a.enumerated() {
                if n > 0 { out += "," }
                v.write(into: &out)
            }
            out += "]"
        case .object(let o):
            out += "{"
            for (n, k) in o.keys.sorted().enumerated() {
                if n > 0 { out += "," }
                JSON.writeString(k, into: &out)
                out += ":"
                o[k]!.write(into: &out)
            }
            out += "}"
        }
    }

    private static func writeString(_ s: String, into out: inout String) {
        out += "\""
        for u in s.unicodeScalars {
            switch u {
            case "\"": out += "\\\""
            case "\\": out += "\\\\"
            case "\n": out += "\\n"
            case "\r": out += "\\r"
            case "\t": out += "\\t"
            case "\u{08}": out += "\\b"
            case "\u{0C}": out += "\\f"
            default:
                if u.value < 0x20 {
                    out += String(format: "\\u%04x", u.value)
                } else {
                    out.unicodeScalars.append(u)
                }
            }
        }
        out += "\""
    }
}

// MARK: - literals (tests and building new items)

extension JSON: ExpressibleByNilLiteral, ExpressibleByBooleanLiteral, ExpressibleByIntegerLiteral, ExpressibleByFloatLiteral, ExpressibleByStringLiteral, ExpressibleByArrayLiteral, ExpressibleByDictionaryLiteral {
    public init(nilLiteral: ()) { self = .null }
    public init(booleanLiteral value: Bool) { self = .bool(value) }
    public init(integerLiteral value: Int64) { self = .int(value) }
    public init(floatLiteral value: Double) { self = .double(value) }
    public init(stringLiteral value: String) { self = .string(value) }
    public init(arrayLiteral elements: JSON...) { self = .array(elements) }
    public init(dictionaryLiteral elements: (String, JSON)...) {
        var o: [String: JSON] = [:]
        for (k, v) in elements { o[k] = v }
        self = .object(o)
    }
}
