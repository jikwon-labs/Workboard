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
            if let name, !name.isBlank { f["name"] = .string(name) }
            if let description { f["description"] = .string(description) }
            if let list { f["list_id"] = .id(list) }
            return true
        }
    }

    @discardableResult
    public static func setFormField(_ data: inout JSON, form: UInt64, index: Int?, to field: FormFieldView) -> Bool {
        guard !field.label.isBlank else { return false }
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
    /// list, logged "submitted the form <name>" for its owner to hear about
    /// and marked with the form (`form`, as the Mac's `Task::form`). The
    /// answers are checked and written as the Mac's form_check does.
    public static func submitForm(_ data: inout JSON, form: UInt64, answers: [Int: String], me: UInt64, id: UInt64, ts: Int64) -> Result<UInt64, FormError> {
        guard let f = WorkspaceView(data).forms(data).first(where: { $0.id == form }) else { return .failure(.noList) }
        var name = "", desc = "", priority = defaultPriority, due: String?, assignees: [UInt64] = []
        for (i, fld) in f.fields.enumerated() {
            let label = fld.label.isBlank ? "Untitled question" : fld.label.trimmed
            var v = (answers[i] ?? "").trimmed
            // a choice removed since it was picked is no answer
            if fld.map == "dropdown", !fld.options.contains(where: { $0.trimmed == v }) { v = "" }
            if v.isEmpty {
                // a dropdown with no choices can't be answered, so it can't hold the form up
                let answerable = fld.map != "dropdown" || fld.options.contains { !$0.isBlank }
                if fld.required && answerable { return .failure(.answer("Please answer \"\(label)\".")) }
                continue
            }
            switch fld.map {
            case "name": name = v
            case "description":
                // a second one starts its own paragraph
                if !desc.isEmpty { desc += "\n\n" }
                desc += v
            case "priority": priority = priorityLabels.firstIndex(of: v) ?? 0
            case "due":
                guard v.count == 10, CustomDate.valid(v) else { return .failure(.answer("\"\(label)\" isn't a date. Write it as YYYY-MM-DD.")) }
                due = v
            case "assignee": if let a = UInt64(v) { assignees.append(a) }
            default:
                if fld.map == "number", formNumber(v) == nil { return .failure(.answer("\"\(label)\" needs a number.")) }
                if fld.map == "email", !formEmailOK(v) { return .failure(.answer("\"\(label)\" needs an email address.")) }
                desc += "\n\(label): \(v)"
            }
        }
        guard rawLists(data)[f.listId] != nil else { return .failure(.noList) }
        addTask(&data, list: f.listId, name: name.isEmpty ? "\(f.name) submission" : name, assignees: assignees, due: due, me: me, id: id, ts: ts)
        data.updateItem(at: "tasks", id: id) { t in
            t["priority"] = .int(Int64(priority))
            t["description"] = .string(desc.trimmed)
            t["form"] = .id(form)
            log(&t, me: me, text: "submitted the form \(f.name)", ts: ts)
        }
        return .success(id)
    }

    /// The form an answer from an app that doesn't record `form` went to,
    /// from the name in its history line: the one form of that name, or,
    /// when several have it, the one whose list the task is in (the Mac's
    /// old_answer_form).
    public static func oldAnswerForm(_ forms: [JSON], name: String, list: UInt64) -> UInt64? {
        let named = forms.filter { $0["name"]?.string == name }
        if named.count == 1 { return named[0]["id"]?.uint }
        let inList = named.filter { $0["list_id"]?.uint == list }
        return inList.count == 1 ? inList[0]["id"]?.uint : nil
    }

    /// A Number answer as the Mac reads one (goals' parse_number): thousands
    /// commas and spaces ignored, finite numbers only.
    static func formNumber(_ s: String) -> Double? {
        let plain = String(s.filter { !$0.isWhitespace && $0 != "," }.map { $0 == "\u{2212}" ? "-" : $0 })
        return Double(plain).flatMap { $0.isFinite ? $0 : nil }
    }

    /// An email address as the team server takes one: something before the
    /// @, a dot inside what follows, no spaces.
    static func formEmailOK(_ e: String) -> Bool {
        guard let at = e.firstIndex(of: "@") else { return false }
        let name = e[..<at], domain = e[e.index(after: at)...]
        return !name.isEmpty && domain.contains(".") && !domain.hasPrefix(".") && !domain.hasSuffix(".")
            && !e.contains(where: \.isWhitespace)
    }
}

enum CustomDate {
    static func valid(_ s: String) -> Bool {
        DayFormat.date(fromDay: s) != nil
    }
}
