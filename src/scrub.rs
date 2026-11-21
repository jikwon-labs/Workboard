//! Taking the personal details out of a picture or video before it is sent:
//! where it was taken (GPS), the camera or phone, its settings and any text
//! tags. The picture and sound themselves are copied untouched, so nothing
//! is re-encoded and nothing gets blurrier.
//!
//! Recognised from the bytes, not the name:
//!   JPEG   APP1 (EXIF, XMP), APP13 (IPTC), comments, other app segments and
//!          anything after the picture (extra MPF pictures, motion-photo
//!          videos) go; the colour profile stays. A turned photo keeps its
//!          turn in a tiny EXIF of its own.
//!   PNG    eXIf, tEXt, zTXt, iTXt and tIME go (a turn is kept as for JPEG).
//!   WebP   the EXIF and XMP chunks go.
//!   HEIF   (HEIC, AVIF) the EXIF and XMP items are blanked where they lie,
//!          so no offset moves; the turn lives elsewhere (irot) and stays.
//!   MOV/MP4 the movie's and each track's udta and meta boxes are blanked and
//!          renamed `free`, where they lie, so no offset moves.
//!
//! Anything else (GIF, BMP, TIFF, plain files), and a file that doesn't parse
//! as what it claims to be, gives None and goes as it is.

/// The file's bytes with the personal details taken out, or None when it
/// isn't a kind this knows (or doesn't parse).
pub fn scrub(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        jpeg(bytes)
    } else if bytes.starts_with(PNG_SIG) {
        png(bytes)
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        webp(bytes)
    } else if bytes.len() >= 8 && ISO_FIRST.contains(&&bytes[4..8]) {
        iso(bytes)
    } else {
        None
    }
}

const PNG_SIG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
/// Boxes an ISO media file (MP4, MOV, HEIF) can start with.
const ISO_FIRST: [&[u8]; 7] = [
    b"ftyp", b"wide", b"free", b"skip", b"mdat", b"moov", b"pnot",
];

fn be16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?))
}
fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}
fn be64(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(b.get(at..at + 8)?.try_into().ok()?))
}
fn le32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

// ---- the turn (EXIF orientation) ------------------------------------------

/// The orientation tag (2..=8) in a TIFF block (EXIF without "Exif\0\0"),
/// or None when it is upright or absent.
fn tiff_turn(t: &[u8]) -> Option<u16> {
    let big = match t.get(0..2)? {
        b"MM" => true,
        b"II" => false,
        _ => return None,
    };
    let u16_at = |at: usize| -> Option<u16> {
        let b: [u8; 2] = t.get(at..at + 2)?.try_into().ok()?;
        Some(if big {
            u16::from_be_bytes(b)
        } else {
            u16::from_le_bytes(b)
        })
    };
    let u32_at = |at: usize| -> Option<u32> {
        let b: [u8; 4] = t.get(at..at + 4)?.try_into().ok()?;
        Some(if big {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        })
    };
    let ifd = u32_at(4)? as usize;
    let n = u16_at(ifd)? as usize;
    for i in 0..n {
        let e = ifd + 2 + i * 12;
        if u16_at(e)? == 0x0112 && u16_at(e + 2)? == 3 {
            let v = u16_at(e + 8)?;
            return (2..=8).contains(&v).then_some(v);
        }
    }
    None
}

/// A TIFF block holding nothing but the orientation tag.
fn tiff_with_turn(turn: u16) -> Vec<u8> {
    let mut t = b"MM\0\x2A\0\0\0\x08".to_vec();
    t.extend_from_slice(&1u16.to_be_bytes()); // one entry
    t.extend_from_slice(&0x0112u16.to_be_bytes()); // orientation
    t.extend_from_slice(&3u16.to_be_bytes()); // SHORT
    t.extend_from_slice(&1u32.to_be_bytes()); // count
    t.extend_from_slice(&turn.to_be_bytes());
    t.extend_from_slice(&[0, 0]); // value padding
    t.extend_from_slice(&[0, 0, 0, 0]); // no next IFD
    t
}

// ---- JPEG -----------------------------------------------------------------

fn jpeg(b: &[u8]) -> Option<Vec<u8>> {
    let mut out = vec![0xFF, 0xD8];
    let mut at = 2;
    let mut turn_kept = false;
    loop {
        // markers may be padded with extra 0xFF bytes
        if *b.get(at)? != 0xFF {
            return None;
        }
        while *b.get(at + 1)? == 0xFF {
            at += 1;
        }
        let marker = b[at + 1];
        match marker {
            0xD9 => {
                // end of the picture: whatever trails it goes
                out.extend_from_slice(&[0xFF, 0xD9]);
                return Some(out);
            }
            0x01 | 0xD0..=0xD7 => {
                out.extend_from_slice(&[0xFF, marker]);
                at += 2;
                continue;
            }
            _ => {}
        }
        let len = be16(b, at + 2)? as usize;
        if len < 2 {
            return None;
        }
        let seg = b.get(at..at + 2 + len)?;
        let body = &seg[4..];
        let keep = match marker {
            0xE0 | 0xEE => true, // JFIF, Adobe colour transform
            0xE2 => body.starts_with(b"ICC_PROFILE\0"),
            0xE1 => {
                if !turn_kept && body.starts_with(b"Exif\0\0") {
                    turn_kept = true;
                    if let Some(turn) = tiff_turn(&body[6..]) {
                        let mut app1 = b"Exif\0\0".to_vec();
                        app1.extend(tiff_with_turn(turn));
                        out.extend_from_slice(&[0xFF, 0xE1]);
                        out.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
                        out.extend(app1);
                    }
                }
                false
            }
            0xE3..=0xED | 0xEF | 0xFE => false, // other app data, comments
            _ => true,
        };
        if keep {
            out.extend_from_slice(seg);
        }
        at += 2 + len;
        if marker == 0xDA {
            // a scan: its coded data runs to the next real marker
            let start = at;
            loop {
                let x = *b.get(at)?;
                if x == 0xFF {
                    let y = *b.get(at + 1)?;
                    if y != 0x00 && !(0xD0..=0xD7).contains(&y) && y != 0xFF {
                        break;
                    }
                    at += if y == 0xFF { 1 } else { 2 };
                } else {
                    at += 1;
                }
            }
            out.extend_from_slice(&b[start..at]);
        }
    }
}

// ---- PNG ------------------------------------------------------------------

fn png(b: &[u8]) -> Option<Vec<u8>> {
    let mut out = PNG_SIG.to_vec();
    let mut at = PNG_SIG.len();
    loop {
        let len = be32(b, at)? as usize;
        let kind: [u8; 4] = b.get(at + 4..at + 8)?.try_into().ok()?;
        let end = at.checked_add(12)?.checked_add(len)?;
        let chunk = b.get(at..end)?;
        match &kind {
            b"eXIf" => {
                let data = &chunk[8..8 + len];
                // some writers put "Exif\0\0" in front, against the spec
                let tiff = data.strip_prefix(b"Exif\0\0").unwrap_or(data);
                if let Some(turn) = tiff_turn(tiff) {
                    png_chunk(&mut out, b"eXIf", &tiff_with_turn(turn));
                }
            }
            b"tEXt" | b"zTXt" | b"iTXt" | b"tIME" => {}
            _ => out.extend_from_slice(chunk),
        }
        at = end;
        if &kind == b"IEND" {
            return Some(out);
        }
    }
}

fn png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let from = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[from..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &x in data {
        c ^= x as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

// ---- WebP -----------------------------------------------------------------

fn webp(b: &[u8]) -> Option<Vec<u8>> {
    let total = (le32(b, 4)? as usize).checked_add(8)?.min(b.len());
    let mut out = b[0..12].to_vec();
    let mut at = 12;
    while at + 8 <= total {
        let kind = &b[at..at + 4];
        let len = le32(b, at + 4)? as usize;
        let end = at.checked_add(8)?.checked_add(len + (len & 1))?;
        let chunk = b.get(at..end.min(total))?;
        match kind {
            b"EXIF" | b"XMP " => {}
            b"VP8X" => {
                let mut c = chunk.to_vec();
                // flags: EXIF 0x08, XMP 0x04
                *c.get_mut(8)? &= !0x0C;
                out.extend(c);
            }
            _ => out.extend_from_slice(chunk),
        }
        at = end;
    }
    let size = (out.len() - 8) as u32;
    out[4..8].copy_from_slice(&size.to_le_bytes());
    Some(out)
}

// ---- ISO media (MP4, MOV, HEIF) -------------------------------------------

/// One box: its type, where its contents start and where it ends.
struct Bx {
    kind: [u8; 4],
    at: usize,
    body: usize,
    end: usize,
}

/// The boxes laid one after another in `b[from..to]`.
fn boxes(b: &[u8], from: usize, to: usize) -> Option<Vec<Bx>> {
    let mut v = Vec::new();
    let mut at = from;
    while at + 8 <= to {
        let size = be32(b, at)? as u64;
        let kind: [u8; 4] = b[at + 4..at + 8].try_into().ok()?;
        let (body, size) = match size {
            0 => (at + 8, (to - at) as u64),
            1 => (at + 16, be64(b, at + 8)?),
            s => (at + 8, s),
        };
        let end = at.checked_add(usize::try_from(size).ok()?)?;
        if end < body || end > to {
            return None;
        }
        v.push(Bx {
            kind,
            at,
            body,
            end,
        });
        at = end;
    }
    Some(v)
}

/// Blanks a box where it lies and calls it `free`, so players skip it and
/// nothing after it moves.
fn blank_box(out: &mut [u8], x: &Bx) {
    out[x.at + 4..x.at + 8].copy_from_slice(b"free");
    out[x.body..x.end].fill(0);
}

fn iso(b: &[u8]) -> Option<Vec<u8>> {
    let mut out = b.to_vec();
    for top in boxes(b, 0, b.len())? {
        match &top.kind {
            b"moov" => {
                for x in boxes(b, top.body, top.end)? {
                    match &x.kind {
                        b"udta" | b"meta" => blank_box(&mut out, &x),
                        b"trak" => {
                            for y in boxes(b, x.body, x.end)? {
                                if matches!(&y.kind, b"udta" | b"meta") {
                                    blank_box(&mut out, &y);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            b"meta" => {
                if handler(b, &top) == Some(*b"pict") {
                    heif_meta(b, &mut out, &top)?;
                } else {
                    blank_box(&mut out, &top);
                }
            }
            b"udta" => blank_box(&mut out, &top),
            _ => {}
        }
    }
    Some(out)
}

/// A meta box's handler type (`pict` for a HEIF picture).
fn handler(b: &[u8], meta: &Bx) -> Option<[u8; 4]> {
    let kids = boxes(b, meta.body + 4, meta.end)?;
    let h = kids.iter().find(|k| &k.kind == b"hdlr")?;
    b.get(h.body + 8..h.body + 12)?.try_into().ok()
}

/// Blanks the EXIF and XMP items of a HEIF picture where they lie.
fn heif_meta(b: &[u8], out: &mut [u8], meta: &Bx) -> Option<()> {
    let kids = boxes(b, meta.body + 4, meta.end)?;
    let mut exif = Vec::new();
    let mut xmp = Vec::new();
    if let Some(iinf) = kids.iter().find(|k| &k.kind == b"iinf") {
        let ver = *b.get(iinf.body)?;
        let first = iinf.body + 4 + if ver == 0 { 2 } else { 4 };
        for e in boxes(b, first, iinf.end)? {
            if &e.kind != b"infe" {
                continue;
            }
            let v = *b.get(e.body)?;
            if v < 2 {
                continue;
            }
            let (id, rest) = if v == 2 {
                (be16(b, e.body + 4)? as u32, e.body + 6)
            } else {
                (be32(b, e.body + 4)?, e.body + 8)
            };
            let kind = b.get(rest + 2..rest + 6)?;
            if kind == b"Exif" {
                exif.push(id);
            } else if kind == b"mime" {
                // item name, then the content type, both nul-terminated
                let tail = b.get(rest + 6..e.end)?;
                let mut parts = tail.split(|&c| c == 0);
                parts.next();
                let ct = parts.next().unwrap_or_default();
                let ct = String::from_utf8_lossy(ct).to_ascii_lowercase();
                if ct.contains("rdf+xml") || ct.contains("xmp") {
                    xmp.push(id);
                }
            }
        }
    }
    if exif.is_empty() && xmp.is_empty() {
        return Some(());
    }
    let iloc = kids.iter().find(|k| &k.kind == b"iloc")?;
    let idat = kids.iter().find(|k| &k.kind == b"idat");
    for (id, ranges) in item_ranges(b, iloc, idat)? {
        let filler: &[u8] = if exif.contains(&id) {
            // a valid, empty EXIF: header offset 0, a TIFF with no entries
            b"\0\0\0\0MM\0\x2A\0\0\0\x08\0\0\0\0\0\0"
        } else if xmp.contains(&id) {
            b"<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"/>"
        } else {
            continue;
        };
        let mut left = filler;
        for (s, e) in ranges {
            out[s..e].fill(if xmp.contains(&id) { b' ' } else { 0 });
            let n = left.len().min(e - s);
            out[s..s + n].copy_from_slice(&left[..n]);
            left = &left[n..];
        }
    }
    Some(())
}

/// Each item's byte ranges in the file, from the iloc box.
fn item_ranges(b: &[u8], iloc: &Bx, idat: Option<&Bx>) -> Option<Vec<(u32, Vec<(usize, usize)>)>> {
    let ver = *b.get(iloc.body)?;
    let sizes = *b.get(iloc.body + 4)?;
    let (off_size, len_size) = ((sizes >> 4) as usize, (sizes & 0x0F) as usize);
    let sizes2 = *b.get(iloc.body + 5)?;
    let base_size = (sizes2 >> 4) as usize;
    let index_size = if ver == 1 || ver == 2 {
        (sizes2 & 0x0F) as usize
    } else {
        0
    };
    let mut at = iloc.body + 6;
    let read = |at: &mut usize, n: usize| -> Option<u64> {
        let v = match n {
            0 => 0,
            4 => be32(b, *at)? as u64,
            8 => be64(b, *at)?,
            _ => return None,
        };
        *at += n;
        Some(v)
    };
    let count = if ver < 2 {
        let c = be16(b, at)? as u32;
        at += 2;
        c
    } else {
        let c = be32(b, at)?;
        at += 4;
        c
    };
    let mut items = Vec::new();
    for _ in 0..count {
        let id = if ver < 2 {
            let i = be16(b, at)? as u32;
            at += 2;
            i
        } else {
            let i = be32(b, at)?;
            at += 4;
            i
        };
        let method = if ver == 1 || ver == 2 {
            let m = be16(b, at)? & 0x0F;
            at += 2;
            m
        } else {
            0
        };
        at += 2; // data reference index
        let base = read(&mut at, base_size)?;
        let extents = be16(b, at)?;
        at += 2;
        // where offsets count from, and how far data can reach
        let (origin, limit) = match (method, idat) {
            (0, _) => (0usize, b.len()),
            (1, Some(d)) => (d.body, d.end),
            _ => (usize::MAX, 0), // by reference: nothing of its own
        };
        let mut ranges = Vec::new();
        for _ in 0..extents {
            read(&mut at, index_size)?;
            let off = read(&mut at, off_size)?;
            let len = read(&mut at, len_size)?;
            if origin == usize::MAX {
                continue;
            }
            let s = origin.checked_add(usize::try_from(base.checked_add(off)?).ok()?)?;
            let e = if len == 0 {
                limit
            } else {
                s.checked_add(usize::try_from(len).ok()?)?
            };
            if s > e || e > limit {
                return None;
            }
            ranges.push((s, e));
        }
        items.push((id, ranges));
    }
    Some(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has(hay: &[u8], needle: &[u8]) -> bool {
        hay.windows(needle.len()).any(|w| w == needle)
    }

    /// A TIFF block with an orientation, a camera make and a GPS tag that
    /// holds a recognisable marker.
    fn tiff_full(turn: u16) -> Vec<u8> {
        let mut t = b"II\x2A\0\x08\0\0\0".to_vec();
        t.extend_from_slice(&3u16.to_le_bytes());
        // orientation
        t.extend_from_slice(&0x0112u16.to_le_bytes());
        t.extend_from_slice(&3u16.to_le_bytes());
        t.extend_from_slice(&1u32.to_le_bytes());
        t.extend_from_slice(&turn.to_le_bytes());
        t.extend_from_slice(&[0, 0]);
        // make: ASCII, count 12, at offset 50
        t.extend_from_slice(&0x010Fu16.to_le_bytes());
        t.extend_from_slice(&2u16.to_le_bytes());
        t.extend_from_slice(&12u32.to_le_bytes());
        t.extend_from_slice(&50u32.to_le_bytes());
        // GPS IFD pointer at 62
        t.extend_from_slice(&0x8825u16.to_le_bytes());
        t.extend_from_slice(&4u16.to_le_bytes());
        t.extend_from_slice(&1u32.to_le_bytes());
        t.extend_from_slice(&62u32.to_le_bytes());
        t.extend_from_slice(&[0, 0, 0, 0]);
        assert_eq!(t.len(), 50);
        t.extend_from_slice(b"SECRETPHONE\0");
        t.extend_from_slice(b"GPS-37.5665N-126.9780E");
        t
    }

    fn jpeg_photo(turn: u16) -> Vec<u8> {
        let img = image::RgbImage::from_fn(16, 8, |x, _| image::Rgb([x as u8 * 16, 0, 0]));
        let mut plain = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut plain)
            .encode_image(&img)
            .unwrap();
        // SOI, then EXIF, XMP and a comment, then the rest; a motion-photo
        // style tail after the end
        let mut exif = b"Exif\0\0".to_vec();
        exif.extend(tiff_full(turn));
        let xmp = b"http://ns.adobe.com/xap/1.0/\0<x:xmpmeta>GPS-XMP-SECRET</x:xmpmeta>".to_vec();
        let mut out = vec![0xFF, 0xD8];
        for (m, body) in [
            (0xE1u8, exif),
            (0xE1, xmp),
            (0xFE, b"SECRET-COMMENT".to_vec()),
        ] {
            out.extend_from_slice(&[0xFF, m]);
            out.extend_from_slice(&((body.len() + 2) as u16).to_be_bytes());
            out.extend(body);
        }
        out.extend_from_slice(&plain[2..]);
        out.extend_from_slice(b"ftypmp42 TRAILING-SECRET-VIDEO");
        out
    }

    #[test]
    fn jpeg_loses_gps_camera_and_tail_but_keeps_its_turn_and_pixels() {
        let src = jpeg_photo(6);
        let out = scrub(&src).unwrap();
        for secret in [
            &b"GPS-37"[..],
            b"SECRETPHONE",
            b"GPS-XMP-SECRET",
            b"SECRET-COMMENT",
            b"TRAILING-SECRET",
        ] {
            assert!(!has(&out, secret), "{}", String::from_utf8_lossy(secret));
        }
        // the turn survives in a fresh EXIF
        let at = out.windows(6).position(|w| w == b"Exif\0\0").unwrap();
        assert_eq!(tiff_turn(&out[at + 6..]), Some(6));
        // same picture, decoded
        let a = image::load_from_memory(&src).unwrap().to_rgb8();
        let b = image::load_from_memory(&out).unwrap().to_rgb8();
        assert_eq!(a, b);
        // and a second pass changes nothing
        assert_eq!(scrub(&out).unwrap(), out);
    }

    #[test]
    fn an_upright_jpeg_keeps_no_exif_at_all() {
        let out = scrub(&jpeg_photo(1)).unwrap();
        assert!(!has(&out, b"Exif\0\0"));
        image::load_from_memory(&out).unwrap();
    }

    #[test]
    fn a_progressive_jpegs_scans_all_survive() {
        // markers between scans (DHT, SOS) must be walked, not cut
        let mut b = vec![0xFF, 0xD8];
        b.extend_from_slice(&[0xFF, 0xDA, 0, 4, 1, 2]);
        b.extend_from_slice(&[9, 0xFF, 0x00, 8, 0xFF, 0xD0, 7]);
        b.extend_from_slice(&[0xFF, 0xC4, 0, 3, 5]);
        b.extend_from_slice(&[0xFF, 0xDA, 0, 3, 6, 4, 4, 0xFF, 0xD9]);
        assert_eq!(scrub(&b).unwrap(), b);
    }

    #[test]
    fn png_loses_text_and_exif_but_keeps_its_turn_and_pixels() {
        let img = image::RgbaImage::from_fn(5, 3, |x, y| image::Rgba([x as u8, y as u8, 9, 255]));
        let mut plain = Vec::new();
        image::codecs::png::PngEncoder::new(&mut plain)
            .write_image(&img, 5, 3, image::ExtendedColorType::Rgba8)
            .unwrap();
        use image::ImageEncoder;
        // put eXIf and text right after IHDR (8 + 25 bytes)
        let mut src = plain[..33].to_vec();
        png_chunk(&mut src, b"eXIf", &tiff_full(8));
        png_chunk(&mut src, b"tEXt", b"Comment\0GPS-TEXT-SECRET");
        png_chunk(
            &mut src,
            b"iTXt",
            b"XML:com.adobe.xmp\0\0\0\0\0GPS-XMP-SECRET",
        );
        src.extend_from_slice(&plain[33..]);
        image::load_from_memory(&src).unwrap();
        let out = scrub(&src).unwrap();
        for secret in [&b"GPS-37"[..], b"SECRETPHONE", b"GPS-TEXT", b"GPS-XMP"] {
            assert!(!has(&out, secret));
        }
        let at = out.windows(4).position(|w| w == b"eXIf").unwrap();
        assert_eq!(tiff_turn(&out[at + 4..]), Some(8));
        let a = image::load_from_memory(&src).unwrap().to_rgba8();
        let b = image::load_from_memory(&out).unwrap().to_rgba8();
        assert_eq!(a, b);
        assert_eq!(scrub(&out).unwrap(), out);
    }

    #[test]
    fn webp_loses_its_exif_and_xmp_chunks() {
        let img = image::RgbaImage::from_fn(4, 4, |x, _| image::Rgba([x as u8 * 60, 0, 0, 255]));
        let mut plain = Vec::new();
        image::codecs::webp::WebPEncoder::new_lossless(&mut plain)
            .encode(&img, 4, 4, image::ExtendedColorType::Rgba8)
            .unwrap();
        // turn it into an extended file with EXIF and XMP
        let mut body = b"WEBP".to_vec();
        let mut vp8x = vec![0x0C, 0, 0, 0, 3, 0, 0, 3, 0, 0];
        vp8x[0] = 0x0C;
        body.extend_from_slice(b"VP8X");
        body.extend_from_slice(&10u32.to_le_bytes());
        body.extend(vp8x);
        body.extend_from_slice(&plain[12..]);
        for (k, d) in [
            (b"EXIF", tiff_full(1)),
            (b"XMP ", b"GPS-XMP-SECRET!".to_vec()),
        ] {
            body.extend_from_slice(k);
            body.extend_from_slice(&(d.len() as u32).to_le_bytes());
            let odd = d.len() & 1;
            body.extend(d);
            if odd == 1 {
                body.push(0);
            }
        }
        let mut src = b"RIFF".to_vec();
        src.extend_from_slice(&(body.len() as u32).to_le_bytes());
        src.extend(body);
        image::load_from_memory(&src).unwrap();
        let out = scrub(&src).unwrap();
        assert!(!has(&out, b"GPS-37") && !has(&out, b"GPS-XMP") && !has(&out, b"SECRETPHONE"));
        assert_eq!(out[20] & 0x0C, 0);
        assert_eq!(le32(&out, 4).unwrap() as usize, out.len() - 8);
        let a = image::load_from_memory(&src).unwrap().to_rgba8();
        let b = image::load_from_memory(&out).unwrap().to_rgba8();
        assert_eq!(a, b);
    }

    fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn a_videos_location_and_phone_go_and_nothing_moves() {
        let keys = bx(
            b"keys",
            b"\0\0\0\0\0\0\0\x01\0\0\0\x2Cmdtacom.apple.quicktime.location.ISO6709",
        );
        let ilst = bx(
            b"ilst",
            &bx(
                b"\0\0\0\x01",
                &bx(b"data", b"\0\0\0\x01\0\0\0\0+37.5665+126.9780/"),
            ),
        );
        let meta = bx(b"meta", &[keys, ilst].concat());
        let udta = bx(b"udta", &bx(b"\xA9xyz", b"\0\x12\0\0+37.5665+126.9780/"));
        let trak = bx(
            b"trak",
            &[bx(b"tkhd", &[7; 20]), bx(b"udta", b"SECRET-TRACK")].concat(),
        );
        let moov = bx(b"moov", &[bx(b"mvhd", &[5; 20]), trak, udta, meta].concat());
        let src = [
            bx(b"ftyp", b"qt  \0\0\0\0qt  "),
            bx(b"mdat", b"FRAMES"),
            moov,
        ]
        .concat();
        let out = scrub(&src).unwrap();
        assert_eq!(out.len(), src.len());
        assert!(!has(&out, b"+37.5665") && !has(&out, b"SECRET-TRACK") && !has(&out, b"location"));
        // the parts players need are untouched
        assert!(has(&out, b"FRAMES") && has(&out, &[7; 20]) && has(&out, &[5; 20]));
        let kinds: Vec<_> = boxes(&out, 0, out.len())
            .unwrap()
            .iter()
            .map(|b| b.kind)
            .collect();
        assert_eq!(kinds, vec![*b"ftyp", *b"mdat", *b"moov"]);
    }

    #[test]
    fn a_heic_loses_its_exif_and_xmp_items_where_they_lie() {
        // ftyp, meta (hdlr pict, iinf with Exif 1 + mime/XMP 2 + hvc1 3,
        // iloc pointing into mdat), mdat
        let exif = [&b"\0\0\0\x06Exif\0\0"[..], &tiff_full(1)].concat();
        let xmp = b"<x:xmpmeta>GPS-XMP-SECRET</x:xmpmeta>".to_vec();
        let pixels = b"HEVC-PIXELS".to_vec();
        let hdlr = bx(b"hdlr", b"\0\0\0\0\0\0\0\0pict\0\0\0\0\0\0\0\0\0\0\0\0\0");
        let infe = |id: u16, kind: &[u8; 4], tail: &[u8]| {
            let mut b = vec![2, 0, 0, 0];
            b.extend_from_slice(&id.to_be_bytes());
            b.extend_from_slice(&[0, 0]);
            b.extend_from_slice(kind);
            b.extend_from_slice(tail);
            bx(b"infe", &b)
        };
        let iinf = bx(
            b"iinf",
            &[
                vec![0, 0, 0, 0, 0, 3],
                infe(1, b"Exif", b"\0"),
                infe(2, b"mime", b"\0application/rdf+xml\0"),
                infe(3, b"hvc1", b"\0"),
            ]
            .concat(),
        );
        // iloc v0, 4-byte offsets and lengths, no base
        let iloc_len = 8 + 4 + 2 + 2 + 3 * (2 + 2 + 2 + 8);
        let ftyp = bx(b"ftyp", b"heic\0\0\0\0mif1heic");
        let meta_len = 8 + 4 + hdlr.len() + iinf.len() + iloc_len;
        let mdat_body = ftyp.len() + meta_len + 8;
        let offs = [
            mdat_body,
            mdat_body + exif.len(),
            mdat_body + exif.len() + xmp.len(),
        ];
        let lens = [exif.len(), xmp.len(), pixels.len()];
        let mut il = vec![0, 0, 0, 0, 0x44, 0x00, 0, 3];
        for i in 0..3 {
            il.extend_from_slice(&(i as u16 + 1).to_be_bytes());
            il.extend_from_slice(&[0, 0, 0, 1]);
            il.extend_from_slice(&(offs[i] as u32).to_be_bytes());
            il.extend_from_slice(&(lens[i] as u32).to_be_bytes());
        }
        let iloc = bx(b"iloc", &il);
        assert_eq!(iloc.len(), iloc_len);
        let meta = bx(b"meta", &[vec![0, 0, 0, 0], hdlr, iinf, iloc].concat());
        let src = [ftyp, meta, bx(b"mdat", &[exif, xmp, pixels].concat())].concat();
        let out = scrub(&src).unwrap();
        assert_eq!(out.len(), src.len());
        assert!(!has(&out, b"GPS-37") && !has(&out, b"SECRETPHONE") && !has(&out, b"GPS-XMP"));
        assert!(has(&out, b"HEVC-PIXELS"));
        // the EXIF item is still a valid, empty one
        let at = offs[0];
        assert_eq!(&out[at..at + 8], b"\0\0\0\0MM\0\x2A");
        assert_eq!(tiff_turn(&out[at + 4..]), None);
    }

    #[test]
    fn other_files_and_broken_ones_go_as_they_are() {
        assert!(scrub(b"GIF89a....").is_none());
        assert!(scrub(b"%PDF-1.7").is_none());
        assert!(scrub(&[0xFF, 0xD8, 0xFF, 0xE1, 0x40]).is_none());
        assert!(scrub(&[PNG_SIG, b"\0\0\0\x0DIHDR"].concat()).is_none());
        let mut bad = bx(b"ftyp", b"isom");
        bad.extend_from_slice(&[0, 0, 0x10, 0, b'm', b'o', b'o', b'v']);
        assert!(scrub(&bad).is_none());
    }
}
