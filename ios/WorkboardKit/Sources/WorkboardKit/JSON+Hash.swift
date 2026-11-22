// FNV-1a fingerprints of JSON trees, fed byte by byte with exactly the bytes
// `serialized()` would write, without building the text. Fingerprints are
// taken of every item on every change, so building a String per item (and
// per field) was most of what stamping and merging cost.

import Foundation

/// A running FNV-1a hash (the same function as Sync.fnv).
struct FNV {
    static let hex: [UInt8] = Array("0123456789abcdef".utf8)
    var h: UInt64 = 0xcbf29ce484222325

    @inline(__always) mutating func byte(_ b: UInt8) {
        h ^= UInt64(b)
        h = h &* 0x100000001b3
    }

    mutating func bytes(_ s: StaticString) {
        s.withUTF8Buffer { for b in $0 { byte(b) } }
    }

    mutating func text(_ s: String) {
        for b in s.utf8 { byte(b) }
    }

    /// The decimal digits of `v`, as String(v) writes them.
    mutating func uint(_ v: UInt64) {
        if v == 0 { byte(0x30); return }
        var digits: (UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8,
                     UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8) =
            (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)
        var n = 0
        var x = v
        withUnsafeMutableBytes(of: &digits) { buf in
            while x > 0 {
                buf[n] = UInt8(0x30 + x % 10)
                x /= 10
                n += 1
            }
            while n > 0 {
                n -= 1
                byte(buf[n])
            }
        }
    }

    mutating func int(_ v: Int64) {
        if v < 0 { byte(0x2D) }
        uint(v.magnitude)
    }

    /// A JSON string with the escapes `serialized()` writes. Escapes only
    /// ever replace ASCII bytes, so walking the UTF-8 bytes gives the same
    /// output as walking the scalars.
    mutating func string(_ s: String) {
        byte(0x22)
        for b in s.utf8 {
            switch b {
            case 0x22: byte(0x5C); byte(0x22)
            case 0x5C: byte(0x5C); byte(0x5C)
            case 0x0A: byte(0x5C); byte(0x6E)
            case 0x0D: byte(0x5C); byte(0x72)
            case 0x09: byte(0x5C); byte(0x74)
            case 0x08: byte(0x5C); byte(0x62)
            case 0x0C: byte(0x5C); byte(0x66)
            case 0..<0x20:
                // \u00XX, lowercase hex like String(format: "\\u%04x")
                bytes("\\u00")
                byte(FNV.hex[Int(b >> 4)])
                byte(FNV.hex[Int(b & 0xF)])
            default: byte(b)
            }
        }
        byte(0x22)
    }

    /// A whole value, byte for byte as `serialized()` writes it.
    mutating func json(_ v: JSON) {
        switch v {
        case .null: bytes("null")
        case .bool(let b): if b { bytes("true") } else { bytes("false") }
        case .int(let i): int(i)
        case .uint(let u): uint(u)
        case .double(let d):
            if d.isFinite { text("\(d)") } else { bytes("null") }
        case .string(let s): string(s)
        case .array(let a):
            byte(0x5B)
            var first = true
            for x in a {
                if !first { byte(0x2C) }
                first = false
                json(x)
            }
            byte(0x5D)
        case .object(let o):
            object(o.keys.sorted(), o)
        }
    }

    /// An object of the given keys (sorted the way `serialized()` sorts them).
    mutating func object(_ sortedKeys: [String], _ o: [String: JSON]) {
        byte(0x7B)
        var first = true
        for k in sortedKeys {
            guard let v = o[k] else { continue }
            if !first { byte(0x2C) }
            first = false
            string(k)
            byte(0x3A)
            json(v)
        }
        byte(0x7D)
    }
}

extension JSON {
    /// fnv(serialized()), without building the text.
    public var fingerprint: UInt64 {
        var f = FNV()
        f.json(self)
        return f.h
    }
}
