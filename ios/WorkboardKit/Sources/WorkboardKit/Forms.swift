// Forms (Form and FormField in src/model.rs, form_answers and Submit in
// src/app/more.rs): questions whose answers make a task in the form's list.

import Foundation

public struct FormFieldView: Hashable, Sendable {
    public let label: String
    /// name, description, priority, due, assignee or text; dropdown,
    /// number, email and checkbox answers go into the description too.
    public let map: String
    public let required: Bool
    public let options: [String]

    init(_ j: JSON) {
        label = j["label"]?.string ?? ""
        map = j["map"]?.string ?? "text"
        required = j["required"]?.bool ?? false
        options = (j["options"]?.array ?? []).compactMap(\.string)
    }

    public init(label: String, map: String, required: Bool = false, options: [String] = []) {
        self.label = label
        self.map = map
        self.required = required
        self.options = options
    }

    var json: JSON {
        var j: JSON = ["label": .string(label), "map": .string(map)]
        if required { j["required"] = true }
        if !options.isEmpty { j["options"] = .array(options.map(JSON.string)) }
        return j
    }

    /// The kinds a question can be, with their names on the Mac.
    public static let kinds: [(map: String, label: String)] = [
        ("name", "Task name"), ("description", "Description"), ("priority", "Priority"), ("due", "Due date"),
        ("assignee", "Assignee"), ("text", "Short text"), ("dropdown", "Dropdown"), ("number", "Number"),
        ("email", "Email"), ("checkbox", "Checkbox"),
    ]
}

public struct FormView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    public let name: String
    public let listId: UInt64
    public let description: String
    public let fields: [FormFieldView]
    public let owner: UInt64

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        name = j["name"]?.string ?? ""
        listId = j["list_id"]?.uint ?? 0
        description = j["description"]?.string ?? ""
        fields = (j["fields"]?.array ?? []).map(FormFieldView.init)
        owner = j["owner"]?.uint ?? 0
    }
}

extension WorkspaceView {
    public func forms(_ data: JSON) -> [FormView] { (data.at("extra.forms")?.array ?? []).map(FormView.init) }
}

extension Edits {
    /// "New Form": Title (the task name) and Details (its description).
    @discardableResult
    public static func addForm(_ data: inout JSON, list: UInt64, me: UInt64, id: UInt64) -> Bool {
        var fs = data.at("extra.forms")?.array ?? []
        fs.append([
            "id": .id(id), "name": "New Form", "list_id": .id(list), "description": "Fill out this form to create a task.",
            "fields": [FormFieldView(label: "Title", map: "name").json, FormFieldView(label: "Details", map: "description").json],
            "owner": .id(me),
        ])
        data.set("extra.forms", .array(fs))
        return true
    }

    private static func changeForm(_ data: inout JSON, _ id: UInt64, _ change: (inout JSON) -> Bool) -> Bool {
        data.tryUpdateItem(at: "extra.forms", id: id, change)
    }

    @discardableResult
    public static func updateForm(_ data: inout JSON, form: UInt64, name: String? = nil, description: String? = nil, list: UInt64? = nil) -> Bool {
        changeForm(&data, form) { f in
            if let name, !name.trimmingCharacters(in: .whitespaces).isEmpty { f["name"] = .string(name) }
            if let description { f["description"] = .string(description) }
            if let list { f["list_id"] = .id(list) }
            return true
        }
    }

    @discardableResult
    public static func setFormField(_ data: inout JSON, form: UInt64, index: Int?, to field: FormFieldView) -> Bool {
        guard !field.label.trimmingCharacters(in: .whitespaces).isEmpty else { return false }
        return changeForm(&data, form) { f in
            var fields = f["fields"]?.array ?? []
            if let index {
                guard fields.indices.contains(index) else { return false }
                fields[index] = field.json
            } else {
                fields.append(field.json)
            }
            f["fields"] = .array(fields)
            return true
        }
    }

    @discardableResult
    public static func removeFormField(_ data: inout JSON, form: UInt64, index: Int) -> Bool {
        changeForm(&data, form) { f in
            var fields = f["fields"]?.array ?? []
            guard fields.indices.contains(index) else { return false }
            fields.remove(at: index)
            f["fields"] = .array(fields)
            return true
        }
    }

    @discardableResult
    public static func deleteForm(_ data: inout JSON, form: UInt64) -> Bool {
        var fs = data.at("extra.forms")?.array ?? []
        let n = fs.count
        fs.removeAll { $0["id"]?.uint == form }
        guard fs.count != n else { return false }
        data.set("extra.forms", .array(fs))
        return true
    }

    public enum FormError: Error, Equatable { case answer(String), noList }

    /// Submit: the answers (by question index) make a task in the form's
    /// list, logged "submitted the form <name>" for its owner to hear about.
    public static func submitForm(_ data: inout JSON, form: UInt64, answers: [Int: String], me: UInt64, id: UInt64, ts: Int64) -> Result<UInt64, FormError> {
        guard let f = WorkspaceView(data).forms(data).first(where: { $0.id == form }) else { return .failure(.noList) }
        var name = "", desc = "", priority = 0, due: String?, assignees: [UInt64] = []
        for (i, fld) in f.fields.enumerated() {
            let v = (answers[i] ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            if v.isEmpty {
                if fld.required { return .failure(.answer("Please answer \"\(fld.label)\".")) }
                continue
            }
            switch fld.map {
            case "name": name = v
            case "description": desc += v
            case "priority": priority = priorityLabels.firstIndex(of: v) ?? 0
            case "due":
                guard v.count == 10, CustomDate.valid(v) else { return .failure(.answer("\"\(fld.label)\" isn't a date. Write it as YYYY-MM-DD.")) }
                due = v
            case "assignee": if let a = UInt64(v) { assignees.append(a) }
            default:
                if fld.map == "number", Double(v) == nil { return .failure(.answer("\"\(fld.label)\" needs a number.")) }
                if fld.map == "email", !(v.contains("@") && v.contains(".")) { return .failure(.answer("\"\(fld.label)\" needs an email address.")) }
                desc += "\n\(fld.label): \(v)"
            }
        }
        guard rawLists(data)[f.listId] != nil else { return .failure(.noList) }
        addTask(&data, list: f.listId, name: name.isEmpty ? "\(f.name) submission" : name, assignees: assignees, due: due, me: me, id: id, ts: ts)
        data.updateItem(at: "tasks", id: id) { t in
            t["priority"] = .int(Int64(priority))
            t["description"] = .string(desc.trimmingCharacters(in: .whitespacesAndNewlines))
            log(&t, me: me, text: "submitted the form \(f.name)", ts: ts)
        }
        return .success(id)
    }
}

enum CustomDate {
    static func valid(_ s: String) -> Bool {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyy-MM-dd"
        return f.date(from: s) != nil
    }
}
