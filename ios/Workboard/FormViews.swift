// Forms: filling one in makes a task; editing its questions (More tab).

import SwiftUI
import WorkboardKit

struct FormsView: View {
    @Environment(SyncClient.self) private var client

    var body: some View {
        let forms = client.data.map { client.view.forms($0) } ?? []
        List {
            if forms.isEmpty {
                EmptyNote(title: "No forms yet", systemImage: "list.clipboard", detail: "A form's answers become a task in its list.").listRowBackground(Color.clear)
            }
            ForEach(forms) { f in
                NavigationLink { FormDetailView(formId: f.id) } label: {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(f.name).fontWeight(.medium)
                        Text("Answers go to \(client.view.list(f.listId)?.name ?? "no list")").font(.caption).foregroundStyle(.secondary)
                    }
                }
                .accessibilityIdentifier("form-\(f.id)")
            }
        }
        .navigationTitle("Forms")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                AddButton("New form", id: "newFormButton") {
                    let id = client.newId()
                    let list = client.view.visibleSpaces(me: client.me ?? 0).flatMap(\.allListIds).first ?? 0
                    client.edit { d, me in Edits.addForm(&d, list: list, me: me, id: id) }
                }
            }
        }
    }
}

struct FormDetailView: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let formId: UInt64
    @State private var editing = false
    @State private var answers: [Int: String] = [:]
    @State private var message: String?
    @State private var failed = false

    var body: some View {
        if let data = client.data, let f = client.view.forms(data).first(where: { $0.id == formId }) {
            Group {
                if editing { FormEditor(form: f) } else { fill(f) }
            }
            .navigationTitle(f.name)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button(editing ? "Done" : "Edit") { editing.toggle() }
                        .disabled(!client.canEdit)
                        .accessibilityIdentifier("formEditToggle")
                }
            }
        } else {
            EmptyNote(title: "This form is gone", systemImage: "list.clipboard")
        }
    }

    func fill(_ f: FormView) -> some View {
        Form {
            if !f.description.isEmpty { Section { Text(f.description).foregroundStyle(.secondary) } }
            ForEach(Array(f.fields.enumerated()), id: \.offset) { i, fld in
                Section {
                    answer(i, fld)
                } header: {
                    Text(fld.label + (fld.required ? " *" : ""))
                }
            }
            Section {
                Button("Submit") {
                    let id = client.newId()
                    var result: Result<UInt64, Edits.FormError> = .failure(.noList)
                    client.edit { d, me in result = Edits.submitForm(&d, form: f.id, answers: answers, me: me, id: id, ts: nowTs()) }
                    switch result {
                    case .success:
                        answers = [:]
                        failed = false
                        message = "Submitted. A task was created in \(client.view.list(f.listId)?.name ?? "its list")."
                    case .failure(.answer(let why)):
                        failed = true
                        message = why
                    case .failure(.noList):
                        failed = true
                        message = "This form has no List for its answers. Choose one under Edit."
                    }
                }
                .disabled(!client.canEdit)
                .accessibilityIdentifier("formSubmit")
                if let message {
                    Text(message).font(.footnote).foregroundStyle(failed ? .red : .green).accessibilityIdentifier("formMessage")
                }
            }
        }
    }

    @ViewBuilder func answer(_ i: Int, _ fld: FormFieldView) -> some View {
        let text = Binding(get: { answers[i] ?? "" }, set: { answers[i] = $0 })
        switch fld.map {
        case "priority":
            Picker("Priority", selection: text) {
                Text("Choose").tag("")
                ForEach(Edits.priorityLabels, id: \.self) { Text(L($0)).tag($0) }
            }
        case "assignee":
            Picker("Person", selection: text) {
                Text("Choose").tag("")
                ForEach(client.view.activeMembers) { Text($0.name).tag(String($0.id)) }
            }
        case "dropdown":
            Picker("Choice", selection: text) {
                Text("Choose").tag("")
                ForEach(fld.options, id: \.self) { Text($0).tag($0) }
            }
        case "checkbox":
            Toggle(fld.label, isOn: Binding(get: { answers[i] == "Yes" }, set: { answers[i] = $0 ? "Yes" : "" }))
        case "due":
            DatePicker("Date", selection: Binding(get: { DayFormat.date(fromDay: answers[i] ?? "") ?? Date() }, set: { answers[i] = dayString($0) }), displayedComponents: .date)
        case "description":
            TextField("Answer", text: text, axis: .vertical).lineLimit(3...).accessibilityIdentifier("formAnswer-\(i)")
        default:
            TextField("Answer", text: text)
                .keyboardType(fld.map == "number" ? .decimalPad : fld.map == "email" ? .emailAddress : .default)
                .textInputAutocapitalization(fld.map == "email" ? .never : .sentences)
                .accessibilityIdentifier("formAnswer-\(i)")
        }
    }
}

private struct FormEditor: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let form: FormView
    @State private var name = ""
    @State private var desc = ""
    @State private var picking = false
    @State private var newLabel = ""
    @State private var newKind = "text"

    var body: some View {
        Form {
            Section("Form") {
                TextField("Name", text: $name).onSubmit { client.edit { d, _ in Edits.updateForm(&d, form: form.id, name: name) } }
                    .accessibilityIdentifier("formName")
                TextField("Description", text: $desc, axis: .vertical).onSubmit { client.edit { d, _ in Edits.updateForm(&d, form: form.id, description: desc) } }
                Button { picking = true } label: {
                    LabeledContent("Answers go to", value: client.view.list(form.listId)?.name ?? "Choose a list")
                }
                .foregroundStyle(.primary)
            }
            Section("Questions") {
                ForEach(Array(form.fields.enumerated()), id: \.offset) { i, f in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(f.label)
                        HStack {
                            Text(L(FormFieldView.kinds.first { $0.map == f.map }?.label ?? f.map)).font(.caption).foregroundStyle(.secondary)
                            Spacer()
                            Toggle("Required", isOn: Binding(get: { f.required }, set: { on in
                                client.edit { d, _ in Edits.setFormField(&d, form: form.id, index: i, to: FormFieldView(label: f.label, map: f.map, required: on, options: f.options)) }
                            }))
                            .labelsHidden()
                            Text("Required").font(.caption).foregroundStyle(.secondary)
                        }
                        if f.map == "dropdown" {
                            Text(f.options.isEmpty ? "No choices yet" : f.options.joined(separator: ", ")).font(.caption).foregroundStyle(.tertiary)
                        }
                    }
                    .swipeActions {
                        Button("Delete", role: .destructive) { client.edit { d, _ in Edits.removeFormField(&d, form: form.id, index: i) } }
                    }
                }
                TextField("New question", text: $newLabel).accessibilityIdentifier("newQuestion")
                Picker("Kind", selection: $newKind) {
                    ForEach(FormFieldView.kinds, id: \.map) { Text(L($0.label)).tag($0.map) }
                }
                Button("Add question") {
                    // a dropdown's choices are written after a colon: "Size: S, M, L"
                    var label = newLabel, options: [String] = []
                    if newKind == "dropdown", let c = newLabel.firstIndex(of: ":") {
                        label = String(newLabel[..<c])
                        options = newLabel[newLabel.index(after: c)...].split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
                    }
                    client.edit { d, _ in Edits.setFormField(&d, form: form.id, index: nil, to: FormFieldView(label: label, map: newKind, options: options)) }
                    newLabel = ""
                }
                .disabled(newLabel.isBlank)
                .accessibilityIdentifier("addQuestion")
            }
            Section {
                Button("Delete form", role: .destructive) {
                    if client.edit({ d, _ in Edits.deleteForm(&d, form: form.id) }) { dismiss() }
                }
            }
        }
        .onAppear { name = form.name; desc = form.description }
        .onDisappear {
            if name != form.name || desc != form.description {
                client.edit { d, _ in Edits.updateForm(&d, form: form.id, name: name, description: desc) }
            }
        }
        .sheet(isPresented: $picking) {
            ListPickerSheet(current: form.listId) { l in client.edit { d, _ in Edits.updateForm(&d, form: form.id, list: l) } }
        }
    }
}
