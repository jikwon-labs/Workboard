// The fake server's WebSocket frames, read and written by hand on the TCP
// connection (see FakeServer.swift).

import Foundation
import Network
import WorkboardKit

extension FakeServer {
    func readFrames(_ p: Peer) {
        while true {
            let b = [UInt8](p.buffer.prefix(14))
            guard b.count >= 2 else { return }
            let fin = b[0] & 0x80 != 0
            let opcode = b[0] & 0x0F
            let masked = b[1] & 0x80 != 0
            var len = UInt64(b[1] & 0x7F)
            var at = 2
            if len == 126 {
                guard b.count >= 4 else { return }
                len = UInt64(b[2]) << 8 | UInt64(b[3])
                at = 4
            } else if len == 127 {
                guard b.count >= 10 else { return }
                len = (2..<10).reduce(UInt64(0)) { $0 << 8 | UInt64(b[$1]) }
                at = 10
            }
            let maskLen = masked ? 4 : 0
            guard UInt64(p.buffer.count) >= UInt64(at + maskLen) + len else { return }
            let start = p.buffer.startIndex
            let mask = masked ? [UInt8](p.buffer[(start + at)..<(start + at + 4)]) : []
            var payload = [UInt8](p.buffer[(start + at + maskLen)..<(start + at + maskLen + Int(len))])
            if masked { for i in payload.indices { payload[i] ^= mask[i % 4] } }
            p.buffer.removeSubrange(start..<(start + at + maskLen + Int(len)))
            switch opcode {
            case 0x0, 0x1, 0x2:
                p.fragments.append(contentsOf: payload)
                if fin {
                    let text = String(decoding: p.fragments, as: UTF8.self)
                    p.fragments = Data()
                    handle(text, from: p)
                }
            case 0x8:
                frame(p, opcode: 0x8, Data(payload.prefix(2)))
                drop(p)
                return
            case 0x9:
                frame(p, opcode: 0xA, Data(payload))
            default:
                break
            }
        }
    }

    func frame(_ p: Peer, opcode: UInt8, _ payload: Data) {
        var head: [UInt8] = [0x80 | opcode]
        let n = payload.count
        if n < 126 {
            head.append(UInt8(n))
        } else if n <= 0xFFFF {
            head += [126, UInt8(n >> 8), UInt8(n & 0xFF)]
        } else {
            head.append(127)
            for s in stride(from: 56, through: 0, by: -8) { head.append(UInt8((UInt64(n) >> UInt64(s)) & 0xFF)) }
        }
        p.conn.send(content: Data(head) + payload, completion: .contentProcessed { _ in })
    }
}
