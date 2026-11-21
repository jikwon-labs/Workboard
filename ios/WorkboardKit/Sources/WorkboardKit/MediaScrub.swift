import Foundation

/// Takes the personal details out of a picture or video before it is sent:
/// where it was taken (GPS), the phone, its settings and any text tags. The
/// picture and sound are copied untouched (nothing is re-encoded). Ported
/// from src/scrub.rs, which says what each kind loses; the two must agree.
///
/// JPEG, PNG and WebP lose their metadata segments (a turned photo keeps
/// its turn); HEIF has its EXIF and XMP items blanked where they lie; a
/// MOV/MP4 has its udta and meta boxes blanked and renamed `free`. Anything
/// else, or a file that doesn't parse, gives nil and goes as it is.
public enum MediaScrub {
    /// The bytes with the personal details taken out, or nil when this isn't
    /// a kind it knows (or doesn't parse).
    public static func scrub(_ data: Data) -> Data? {
        let b = [UInt8](data)
        let out: [UInt8]?
        if b.starts(with: [0xFF, 0xD8, 0xFF]) {
            out = jpeg(b)
        } else if b.starts(with: pngSig) {
            out = png(b)
        } else if b.count >= 12, b[0..<4].elementsEqual(Array("RIFF".utf8)), b[8..<12].elementsEqual(Array("WEBP".utf8)) {
            out = webp(b)
        } else if b.count >= 8, isoFirst.contains(where: { b[4..<8].elementsEqual(Array($0.utf8)) }) {
            out = iso(b)
        } else {
            out = nil
        }
        return out.map { Data($0) }
    }

    /// File name endings taken as pictures and videos, as on the Mac
    /// (IMAGE_EXTS and VIDEO_EXTS in src/media.rs): these are the files
    /// read and scrubbed before going up; any other file goes as it is.
    public static let mediaExts: Set<String> = [
        "jpg", "jpeg", "png", "gif", "heic", "heif", "webp", "tif", "tiff", "bmp",
        "mov", "mp4", "m4v", "3gp",
    ]

    /// Scrubs the file at `url` in place. True when something was taken out.
    @discardableResult
    public static func scrubFile(at url: URL) throws -> Bool {
        let data = try Data(contentsOf: url)
        guard let clean = scrub(data), clean != data else { return false }
        try clean.write(to: url, options: .atomic)
        return true
    }

    static let pngSig: [UInt8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
    static let isoFirst = ["ftyp", "wide", "free", "skip", "mdat", "moov", "pnot"]

    static func be16(_ b: [UInt8], _ at: Int) -> Int? {
        guard at >= 0, at + 2 <= b.count else { return nil }
        return Int(b[at]) << 8 | Int(b[at + 1])
    }
    static func be32(_ b: [UInt8], _ at: Int) -> Int? {
        guard at >= 0, at + 4 <= b.count else { return nil }
        return Int(b[at]) << 24 | Int(b[at + 1]) << 16 | Int(b[at + 2]) << 8 | Int(b[at + 3])
    }
    static func be64(_ b: [UInt8], _ at: Int) -> Int? {
        guard let hi = be32(b, at), let lo = be32(b, at + 4), hi < 0x7FFF_FFFF else { return nil }
        return hi << 32 | lo
    }
    static func le32(_ b: [UInt8], _ at: Int) -> Int? {
        guard at >= 0, at + 4 <= b.count else { return nil }
        return Int(b[at]) | Int(b[at + 1]) << 8 | Int(b[at + 2]) << 16 | Int(b[at + 3]) << 24
    }
    static func bytes(_ s: String) -> [UInt8] { Array(s.utf8) }
    static func has(_ b: ArraySlice<UInt8>, prefix: [UInt8]) -> Bool { b.starts(with: prefix) }

    // MARK: the turn (EXIF orientation)

    /// The orientation (2...8) in a TIFF block, or nil when upright or absent.
    static func tiffTurn(_ t: ArraySlice<UInt8>) -> Int? {
        let t = Array(t)
        guard t.count >= 8 else { return nil }
        let big: Bool
        if t[0] == 0x4D, t[1] == 0x4D { big = true } else if t[0] == 0x49, t[1] == 0x49 { big = false } else { return nil }
        func u16(_ at: Int) -> Int? {
            guard at >= 0, at + 2 <= t.count else { return nil }
            return big ? Int(t[at]) << 8 | Int(t[at + 1]) : Int(t[at + 1]) << 8 | Int(t[at])
        }
        func u32(_ at: Int) -> Int? {
            guard let a = u16(at), let c = u16(at + 2) else { return nil }
            return big ? a << 16 | c : c << 16 | a
        }
        guard let ifd = u32(4), let n = u16(ifd) else { return nil }
        for i in 0..<n {
            let e = ifd + 2 + i * 12
            guard let tag = u16(e), let type = u16(e + 2) else { return nil }
            if tag == 0x0112, type == 3 {
                guard let v = u16(e + 8) else { return nil }
                return (2...8).contains(v) ? v : nil
            }
        }
        return nil
    }

    /// A TIFF block holding nothing but the orientation.
    static func tiffWithTurn(_ turn: Int) -> [UInt8] {
        [0x4D, 0x4D, 0, 0x2A, 0, 0, 0, 8, 0, 1, 0x01, 0x12, 0, 3, 0, 0, 0, 1,
         UInt8(turn >> 8), UInt8(turn & 0xFF), 0, 0, 0, 0, 0, 0]
    }

    // MARK: JPEG

    static func jpeg(_ b: [UInt8]) -> [UInt8]? {
        var out: [UInt8] = [0xFF, 0xD8]
        var at = 2
        var turnKept = false
        let exifHead = bytes("Exif") + [0, 0]
        while true {
            guard at + 1 < b.count, b[at] == 0xFF else { return nil }
            while at + 1 < b.count, b[at + 1] == 0xFF { at += 1 }
            guard at + 1 < b.count else { return nil }
            let marker = b[at + 1]
            if marker == 0xD9 {
                out += [0xFF, 0xD9]
                return out
            }
            if marker == 0x01 || (0xD0...0xD7).contains(marker) {
                out += [0xFF, marker]
                at += 2
                continue
            }
            guard let len = be16(b, at + 2), len >= 2, at + 2 + len <= b.count else { return nil }
            let seg = b[at..<(at + 2 + len)]
            let body = b[(at + 4)..<(at + 2 + len)]
            var keep: Bool
            switch marker {
            case 0xE0, 0xEE: keep = true
            case 0xE2: keep = has(body, prefix: bytes("ICC_PROFILE") + [0])
            case 0xE1:
                if !turnKept, has(body, prefix: exifHead) {
                    turnKept = true
                    if let turn = tiffTurn(body.dropFirst(6)) {
                        let app1 = exifHead + tiffWithTurn(turn)
                        out += [0xFF, 0xE1, UInt8((app1.count + 2) >> 8), UInt8((app1.count + 2) & 0xFF)]
                        out += app1
                    }
                }
                keep = false
            case 0xE3...0xED, 0xEF, 0xFE: keep = false
            default: keep = true
            }
            if keep { out += seg }
            at += 2 + len
            if marker == 0xDA {
                let start = at
                while true {
                    guard at + 1 < b.count else { return nil }
                    if b[at] == 0xFF {
                        let y = b[at + 1]
                        if y != 0x00, !(0xD0...0xD7).contains(y), y != 0xFF { break }
                        at += y == 0xFF ? 1 : 2
                    } else {
                        at += 1
                    }
                }
                out += b[start..<at]
            }
        }
    }

    // MARK: PNG

    static func png(_ b: [UInt8]) -> [UInt8]? {
        var out = pngSig
        var at = pngSig.count
        while true {
            guard let len = be32(b, at), at + 12 + len <= b.count else { return nil }
            let kind = String(decoding: b[(at + 4)..<(at + 8)], as: UTF8.self)
            let end = at + 12 + len
            switch kind {
            case "eXIf":
                var data = b[(at + 8)..<(at + 8 + len)]
                if has(data, prefix: bytes("Exif") + [0, 0]) { data = data.dropFirst(6) }
                if let turn = tiffTurn(data) { pngChunk(&out, "eXIf", tiffWithTurn(turn)) }
            case "tEXt", "zTXt", "iTXt", "tIME": break
            default: out += b[at..<end]
            }
            at = end
            if kind == "IEND" { return out }
        }
    }

    static func pngChunk(_ out: inout [UInt8], _ kind: String, _ data: [UInt8]) {
        let n = data.count
        out += [UInt8(n >> 24 & 0xFF), UInt8(n >> 16 & 0xFF), UInt8(n >> 8 & 0xFF), UInt8(n & 0xFF)]
        let typed = bytes(kind) + data
        out += typed
        let c = crc32(typed)
        out += [UInt8(c >> 24 & 0xFF), UInt8(c >> 16 & 0xFF), UInt8(c >> 8 & 0xFF), UInt8(c & 0xFF)]
    }

    static func crc32(_ data: [UInt8]) -> UInt32 {
        var c: UInt32 = 0xFFFF_FFFF
        for x in data {
            c ^= UInt32(x)
            for _ in 0..<8 { c = c & 1 != 0 ? 0xEDB8_8320 ^ (c >> 1) : c >> 1 }
        }
        return ~c
    }

    // MARK: WebP

    static func webp(_ b: [UInt8]) -> [UInt8]? {
        guard let riff = le32(b, 4) else { return nil }
        let total = min(riff + 8, b.count)
        var out = Array(b[0..<12])
        var at = 12
        while at + 8 <= total {
            let kind = String(decoding: b[at..<(at + 4)], as: UTF8.self)
            guard let len = le32(b, at + 4) else { return nil }
            let end = at + 8 + len + (len & 1)
            let chunk = b[at..<min(end, total)]
            switch kind {
            case "EXIF", "XMP ": break
            case "VP8X":
                var c = Array(chunk)
                guard c.count > 8 else { return nil }
                c[8] &= ~0x0C
                out += c
            default: out += chunk
            }
            at = end
        }
        let size = out.count - 8
        out[4] = UInt8(size & 0xFF); out[5] = UInt8(size >> 8 & 0xFF)
        out[6] = UInt8(size >> 16 & 0xFF); out[7] = UInt8(size >> 24 & 0xFF)
        return out
    }

    // MARK: ISO media (MP4, MOV, HEIF)

    struct Bx {
        let kind: String
        let at: Int
        let body: Int
        let end: Int
    }

    static func boxes(_ b: [UInt8], _ from: Int, _ to: Int) -> [Bx]? {
        var v: [Bx] = []
        var at = from
        while at + 8 <= to {
            guard let size32 = be32(b, at) else { return nil }
            let kind = String(decoding: b[(at + 4)..<(at + 8)], as: UTF8.self)
            var body = at + 8
            var size = size32
            if size32 == 0 {
                size = to - at
            } else if size32 == 1 {
                guard let big = be64(b, at + 8) else { return nil }
                body = at + 16
                size = big
            }
            let end = at + size
            guard end >= body, end <= to else { return nil }
            v.append(Bx(kind: kind, at: at, body: body, end: end))
            at = end
        }
        return v
    }

    static func blank(_ out: inout [UInt8], _ x: Bx) {
        out.replaceSubrange((x.at + 4)..<(x.at + 8), with: bytes("free"))
        for i in x.body..<x.end { out[i] = 0 }
    }

    static func iso(_ b: [UInt8]) -> [UInt8]? {
        var out = b
        guard let tops = boxes(b, 0, b.count) else { return nil }
        for top in tops {
            switch top.kind {
            case "moov":
                guard let kids = boxes(b, top.body, top.end) else { return nil }
                for x in kids {
                    if x.kind == "udta" || x.kind == "meta" {
                        blank(&out, x)
                    } else if x.kind == "trak" {
                        guard let inner = boxes(b, x.body, x.end) else { return nil }
                        for y in inner where y.kind == "udta" || y.kind == "meta" { blank(&out, y) }
                    }
                }
            case "meta":
                if handler(b, top) == "pict" {
                    guard heifMeta(b, &out, top) else { return nil }
                } else {
                    blank(&out, top)
                }
            case "udta":
                blank(&out, top)
            default:
                break
            }
        }
        return out
    }

    static func handler(_ b: [UInt8], _ meta: Bx) -> String? {
        guard let kids = boxes(b, meta.body + 4, meta.end),
              let h = kids.first(where: { $0.kind == "hdlr" }),
              h.body + 12 <= b.count else { return nil }
        return String(decoding: b[(h.body + 8)..<(h.body + 12)], as: UTF8.self)
    }

    static func heifMeta(_ b: [UInt8], _ out: inout [UInt8], _ meta: Bx) -> Bool {
        guard let kids = boxes(b, meta.body + 4, meta.end) else { return false }
        var exif: Set<Int> = []
        var xmp: Set<Int> = []
        if let iinf = kids.first(where: { $0.kind == "iinf" }) {
            guard iinf.body < b.count else { return false }
            let first = iinf.body + 4 + (b[iinf.body] == 0 ? 2 : 4)
            guard let entries = boxes(b, first, iinf.end) else { return false }
            for e in entries where e.kind == "infe" {
                guard e.body < b.count else { return false }
                let v = b[e.body]
                if v < 2 { continue }
                guard let id = v == 2 ? be16(b, e.body + 4) : be32(b, e.body + 4) else { return false }
                let rest = e.body + (v == 2 ? 6 : 8)
                guard rest + 6 <= e.end else { return false }
                let kind = String(decoding: b[(rest + 2)..<(rest + 6)], as: UTF8.self)
                if kind == "Exif" {
                    exif.insert(id)
                } else if kind == "mime" {
                    let parts = b[(rest + 6)..<e.end].split(separator: 0, omittingEmptySubsequences: false)
                    let ct = parts.count > 1 ? String(decoding: parts[1], as: UTF8.self).lowercased() : ""
                    if ct.contains("rdf+xml") || ct.contains("xmp") { xmp.insert(id) }
                }
            }
        }
        if exif.isEmpty, xmp.isEmpty { return true }
        guard let iloc = kids.first(where: { $0.kind == "iloc" }),
              let items = itemRanges(b, iloc, kids.first(where: { $0.kind == "idat" })) else { return false }
        let emptyExif: [UInt8] = [0, 0, 0, 0, 0x4D, 0x4D, 0, 0x2A, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0]
        let emptyXmp = bytes("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"/>")
        for (id, ranges) in items {
            let isXmp = xmp.contains(id)
            guard exif.contains(id) || isXmp else { continue }
            var left = ArraySlice(isXmp ? emptyXmp : emptyExif)
            for (s, e) in ranges {
                for i in s..<e { out[i] = isXmp ? 0x20 : 0 }
                let n = min(left.count, e - s)
                out.replaceSubrange(s..<(s + n), with: left.prefix(n))
                left = left.dropFirst(n)
            }
        }
        return true
    }

    static func itemRanges(_ b: [UInt8], _ iloc: Bx, _ idat: Bx?) -> [(Int, [(Int, Int)])]? {
        guard iloc.body + 6 <= b.count else { return nil }
        let ver = b[iloc.body]
        let offSize = Int(b[iloc.body + 4] >> 4), lenSize = Int(b[iloc.body + 4] & 0x0F)
        let baseSize = Int(b[iloc.body + 5] >> 4)
        let indexSize = ver == 1 || ver == 2 ? Int(b[iloc.body + 5] & 0x0F) : 0
        var at = iloc.body + 6
        func read(_ n: Int) -> Int? {
            let v: Int?
            switch n {
            case 0: v = 0
            case 4: v = be32(b, at)
            case 8: v = be64(b, at)
            default: v = nil
            }
            if v != nil { at += n }
            return v
        }
        guard let count = ver < 2 ? be16(b, at) : be32(b, at) else { return nil }
        at += ver < 2 ? 2 : 4
        var items: [(Int, [(Int, Int)])] = []
        for _ in 0..<count {
            guard let id = ver < 2 ? be16(b, at) : be32(b, at) else { return nil }
            at += ver < 2 ? 2 : 4
            var method = 0
            if ver == 1 || ver == 2 {
                guard let m = be16(b, at) else { return nil }
                method = m & 0x0F
                at += 2
            }
            at += 2 // data reference index
            guard let base = read(baseSize), let extents = be16(b, at) else { return nil }
            at += 2
            var origin: Int? = nil
            var limit = 0
            if method == 0 {
                origin = 0; limit = b.count
            } else if method == 1, let d = idat {
                origin = d.body; limit = d.end
            }
            var ranges: [(Int, Int)] = []
            for _ in 0..<extents {
                guard read(indexSize) != nil, let off = read(offSize), let len = read(lenSize) else { return nil }
                guard let o = origin else { continue }
                let s = o + base + off
                let e = len == 0 ? limit : s + len
                guard s <= e, e <= limit else { return nil }
                ranges.append((s, e))
            }
            items.append((id, ranges))
        }
        return items
    }
}
