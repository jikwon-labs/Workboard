// New ids, the same rule as Data::new_id in src/model.rs while shared:
// milliseconds * 1024 + tag, always above the last id made here. The tag is
// given by the server in `welcome`, so ids made on different devices never
// collide.

import Foundation

public struct IdMaker: Sendable {
    public var tag: UInt64
    public private(set) var last: UInt64

    public init(tag: UInt64, last: UInt64 = 0) {
        self.tag = tag % 1024
        self.last = last
    }

    public mutating func next(nowMillis: UInt64 = UInt64(max(0, Date().timeIntervalSince1970 * 1000))) -> UInt64 {
        var id = nowMillis * 1024 + tag
        if id <= last {
            id = (last / 1024 + 1) * 1024 + tag
        }
        last = id
        return id
    }
}

/// Seconds since 1970, like now_ts() in src/model.rs: message `ts`, task
/// `created`/`updated`, activity lines.
public func nowTs() -> Int64 { Int64(Date().timeIntervalSince1970) }

/// Milliseconds since 1970: sync stamps (the Mac app and the server stamp in
/// milliseconds, so a stamp in seconds would always lose).
public func nowMs() -> Int64 { Int64((Date().timeIntervalSince1970 * 1000).rounded(.down)) }
