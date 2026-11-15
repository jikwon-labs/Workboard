// A small made-up workspace in the Mac app's data.json shape, for the fake
// server and tests. It also carries fields the iOS app does not know about
// ("mac_only..."), to check they survive.

import Foundation
import WorkboardKit

public enum SampleWorkspace {
    public static func make(now: Date = Date()) -> JSON {
        let t = Int64(now.timeIntervalSince1970)
        let day: (Int) -> String = { dayString(now.addingTimeInterval(Double($0) * 86400)) }
        let statuses: JSON = [
            ["name": "to do", "color": [135, 144, 158], "kind": "Open"],
            ["name": "in progress", "color": [8, 128, 234], "kind": "Custom"],
            ["name": "review", "color": [248, 174, 0], "kind": "Custom"],
            ["name": "complete", "color": [41, 151, 100], "kind": "Closed"],
        ]
        func list(_ id: Int64, _ name: String) -> JSON {
            ["id": .int(id), "name": .string(name), "color": nil, "start": nil, "end": nil, "priority": 0, "owner": nil, "description": "", "archived": false]
        }
        func task(_ id: Int64, _ list: Int64, _ name: String, _ status: String, assignees: [Int64], due: String?, priority: Int64 = 0, desc: String = "") -> JSON {
            [
                "id": .int(id), "list_id": .int(list), "parent": nil, "name": .string(name), "description": .string(desc),
                "status": .string(status), "priority": .int(priority), "assignees": .array(assignees.map { .int($0) }),
                "start": nil, "due": due.map { .string($0) } ?? .null, "tags": [], "checklist": [],
                "activity": [["ts": .int(t - 86400 * 3), "author": 1, "text": "created this task", "comment": false, "post": false]],
                "time_entries": [], "timer_started": nil, "estimate_min": nil,
                "created": .int(t - 86400 * 3), "updated": .int(t - 86400), "order": .int(id),
                "custom": [:], "recur": nil, "archived": false, "done_at": nil, "closed_at": nil,
                "mac_only_task_field": ["kept": true],
            ]
        }
        func msg(_ id: Int64, _ author: Int64, _ text: String, _ ago: Int64, replies: [JSON] = []) -> JSON {
            [
                "id": .int(id), "author": .int(author), "text": .string(text), "ts": .int(t - ago),
                "reactions": [], "replies": .array(replies), "saved": false, "task": nil,
                "post_title": nil, "post_type": "", "pinned": false, "call": nil,
            ]
        }
        func member(_ id: Int64, _ name: String, _ color: JSON, _ role: String, email: String = "") -> JSON {
            ["id": .int(id), "name": .string(name), "color": color, "email": .string(email), "role": .string(role), "invited": false, "status_text": ""]
        }
        func doc(_ id: Int64, _ title: String, _ body: String) -> JSON {
            ["id": .int(id), "space": 100, "owner": 1, "private": false, "archived": false, "meeting_note": false, "parent": nil, "title": .string(title), "body": .string(body), "created": .int(t - 86400 * 5), "updated": .int(t - 3600)]
        }
        return [
            "workspace": "Sample Team",
            "spaces": [[
                "id": 100, "name": "Product", "color": [95, 85, 238], "statuses": statuses,
                "folders": [["id": 110, "name": "Launch", "lists": [list(111, "Website"), list(112, "App")], "expanded": true]],
                "lists": [list(120, "Backlog")],
                "expanded": true, "private_owner": nil, "archived": false, "hidden": false, "icon": "",
            ]],
            "tasks": [
                task(201, 111, "Write the launch post", "in progress", assignees: [1], due: day(0), priority: 2, desc: "Draft for the blog and the newsletter."),
                task(202, 111, "Check pricing page copy", "to do", assignees: [1, 2], due: day(-2), priority: 1),
                task(203, 112, "Fix login crash", "review", assignees: [2], due: day(1)),
                task(204, 112, "App Store screenshots", "to do", assignees: [1], due: day(3)),
                task(205, 120, "Dark mode for settings", "to do", assignees: [], due: nil, priority: 4),
                task(206, 120, "Old onboarding flow", "complete", assignees: [1], due: day(-5)),
            ],
            "members": [
                member(1, "Jamie Park", [95, 85, 238], "Owner"),
                member(2, "Alex Kim", [16, 144, 224], "Member"),
                member(3, "Sam Lee", [225, 107, 22], "Member", email: "sam@example.com"), // only Sam has an email on record
            ],
            "me": 1,
            "next_id": 1000,
            "favorites": [],
            "dark": false,
            "docs": [
                doc(401, "Launch plan", "Goals\n\n- Ship the website on Friday\n- Post on the blog\n\nOwners: Jamie, Alex"),
                doc(402, "Meeting notes", "Weekly sync\n\nDecided to move the beta by one week."),
            ],
            "channels": [
                [
                    "id": 301, "name": "general", "kind": "Channel", "topic": "Everyone", "members": [1, 2, 3],
                    "messages": [
                        msg(311, 2, "Morning! The staging build is up.", 7200, replies: [msg(312, 3, "Looks good on my phone.", 7000)]),
                        msg(313, 3, "Can someone review the pricing copy?", 1800),
                    ],
                    "last_read": .int(t - 7100), "private": false, "list": nil,
                ],
                [
                    "id": 302, "name": "design", "kind": "Channel", "topic": "", "members": [1, 2],
                    "messages": [msg(321, 1, "New icons are in the shared folder.", 86400)],
                    "last_read": .int(t), "private": false, "list": nil,
                ],
                [
                    "id": 303, "name": "", "kind": "Direct", "topic": "", "members": [1, 2],
                    "messages": [msg(331, 2, "Got a minute for the login crash?", 600)],
                    "last_read": 0, "private": false, "list": nil,
                ],
            ],
            "extra": [
                "goals": [], "forms": [], "whiteboards": [], "meetings": [], "fields": [], "automations": [], "templates": [], "dashboards": [], "teams": [],
                "notepad": "", "accent": "black",
                "sync": ["stamps": [:], "tombs": [:], "revived": [:], "hashes": [:]],
                "mac_only_setting": "kept",
            ],
            "mac_only_top": ["nested": [1, 2, 3]],
        ]
    }
}
