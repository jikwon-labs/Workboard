// The doc body editor: a text view that shows the formatting while typing
// (big headings, bold, italics, struck-through done items, code, links,
// faint Markdown marks) with a formatting bar above the keyboard. The text
// stays Markdown, like the Mac (WorkboardKit/DocFormat.swift).

import SwiftUI
import UIKit
import WorkboardKit

struct DocEditor: UIViewRepresentable {
    @Binding var text: String
    /// Set to true to put the cursor in the editor; it turns back off.
    @Binding var focus: Bool

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeUIView(context: Context) -> UITextView {
        let tv = UITextView()
        tv.delegate = context.coordinator
        tv.backgroundColor = .clear
        tv.font = Coordinator.baseFont
        tv.adjustsFontForContentSizeCategory = true
        tv.textContainerInset = UIEdgeInsets(top: 8, left: 15, bottom: 24, right: 15)
        tv.keyboardDismissMode = .interactive
        tv.accessibilityIdentifier = "docEditor"
        tv.inputAccessoryView = context.coordinator.makeBar()
        tv.text = text
        context.coordinator.restyle(tv)
        return tv
    }

    func updateUIView(_ tv: UITextView, context: Context) {
        context.coordinator.parent = self
        if tv.text != text && tv.markedTextRange == nil {
            tv.text = text
            context.coordinator.restyle(tv)
        }
        if focus {
            DispatchQueue.main.async {
                tv.becomeFirstResponder()
                focus = false
            }
        }
    }

    final class Coordinator: NSObject, UITextViewDelegate {
        var parent: DocEditor
        weak var textView: UITextView?
        /// a bar action is replacing the text: not a typed Return
        var applying = false

        init(_ parent: DocEditor) { self.parent = parent }

        static var baseFont: UIFont { UIFont.preferredFont(forTextStyle: .body) }

        /// The bar's buttons: (action, symbol or short text, spoken name).
        static let buttons: [(DocFormat.Action, String, String)] = [
            (.heading1, "H1", String(localized: "Heading 1")),
            (.heading2, "H2", String(localized: "Heading 2")),
            (.heading3, "H3", String(localized: "Heading 3")),
            (.bold, "bold", String(localized: "Bold")),
            (.italic, "italic", String(localized: "Italic")),
            (.strike, "strikethrough", String(localized: "Strikethrough")),
            (.checklist, "checklist", String(localized: "Checklist")),
            (.bullet, "list.bullet", String(localized: "Bulleted list")),
            (.numbered, "list.number", String(localized: "Numbered list")),
            (.quote, "text.quote", String(localized: "Quote")),
            (.link, "link", String(localized: "Link")),
            (.code, "chevron.left.forwardslash.chevron.right", String(localized: "Inline code")),
            (.codeBlock, "curlybraces", String(localized: "Code block")),
            (.divider, "minus", String(localized: "Divider")),
        ]

        func makeBar() -> UIView {
            let bar = UIInputView(frame: CGRect(x: 0, y: 0, width: 320, height: 46), inputViewStyle: .keyboard)
            bar.allowsSelfSizing = true
            let scroll = UIScrollView()
            scroll.showsHorizontalScrollIndicator = false
            scroll.translatesAutoresizingMaskIntoConstraints = false
            let stack = UIStackView()
            stack.axis = .horizontal
            stack.spacing = 2
            stack.translatesAutoresizingMaskIntoConstraints = false
            for (action, icon, name) in Self.buttons {
                var cfg = UIButton.Configuration.plain()
                if icon.count <= 2 {
                    cfg.attributedTitle = AttributedString(icon, attributes: AttributeContainer([.font: UIFont.systemFont(ofSize: 16, weight: .semibold)]))
                } else {
                    cfg.image = UIImage(systemName: icon)
                }
                cfg.baseForegroundColor = .label
                let b = UIButton(configuration: cfg, primaryAction: UIAction { [weak self] _ in self?.apply(action) })
                b.accessibilityLabel = name
                b.accessibilityIdentifier = "fmt-\(action.rawValue)"
                b.widthAnchor.constraint(greaterThanOrEqualToConstant: 42).isActive = true
                stack.addArrangedSubview(b)
            }
            scroll.addSubview(stack)
            bar.addSubview(scroll)
            var done = UIButton.Configuration.plain()
            done.image = UIImage(systemName: "keyboard.chevron.compact.down")
            done.baseForegroundColor = .secondaryLabel
            let hide = UIButton(configuration: done, primaryAction: UIAction { [weak self] _ in self?.textView?.resignFirstResponder() })
            hide.accessibilityLabel = String(localized: "Hide keyboard")
            hide.accessibilityIdentifier = "fmt-hideKeyboard"
            hide.translatesAutoresizingMaskIntoConstraints = false
            bar.addSubview(hide)
            NSLayoutConstraint.activate([
                scroll.leadingAnchor.constraint(equalTo: bar.leadingAnchor, constant: 4),
                scroll.trailingAnchor.constraint(equalTo: hide.leadingAnchor),
                scroll.topAnchor.constraint(equalTo: bar.topAnchor),
                scroll.bottomAnchor.constraint(equalTo: bar.bottomAnchor),
                stack.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor),
                stack.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor),
                stack.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
                stack.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor),
                stack.heightAnchor.constraint(equalTo: scroll.frameLayoutGuide.heightAnchor),
                hide.trailingAnchor.constraint(equalTo: bar.trailingAnchor, constant: -4),
                hide.centerYAnchor.constraint(equalTo: bar.centerYAnchor),
                hide.widthAnchor.constraint(equalToConstant: 44),
            ])
            return bar
        }

        /// A bar button: rewrites the text as Markdown and moves the cursor.
        func apply(_ action: DocFormat.Action) {
            guard let tv = textView else { return }
            if tv.markedTextRange != nil { tv.unmarkText() }
            let (new, sel) = DocFormat.apply(action, to: tv.text, selection: tv.selectedRange)
            replaceAll(tv, with: new, selection: sel)
        }

        /// Swaps the whole text through the text input system, so Undo
        /// (shake or three-finger swipe) can take the change back.
        func replaceAll(_ tv: UITextView, with new: String, selection: NSRange) {
            applying = true
            if let all = tv.textRange(from: tv.beginningOfDocument, to: tv.endOfDocument) {
                tv.replace(all, withText: new)
            }
            applying = false
            tv.selectedRange = selection
            parent.text = tv.text
            restyle(tv)
            tv.scrollRangeToVisible(selection)
        }

        func textViewDidBeginEditing(_ textView: UITextView) { self.textView = textView }

        func textViewDidChange(_ tv: UITextView) {
            textView = tv
            parent.text = tv.text
            // restyling while Korean is being composed would cut the syllable
            if tv.markedTextRange == nil { restyle(tv) }
        }

        func textView(_ tv: UITextView, shouldChangeTextIn range: NSRange, replacementText rep: String) -> Bool {
            textView = tv
            // Return on a list item starts the next one (or ends an empty one)
            guard rep == "\n", !applying, tv.markedTextRange == nil else { return true }
            let ns = tv.text as NSString
            let without = ns.replacingCharacters(in: range, with: "")
            let (new, caret) = DocFormat.newline(in: without, at: range.location)
            if new == ns.replacingCharacters(in: range, with: "\n") { return true }
            replaceAll(tv, with: new, selection: NSRange(location: caret, length: 0))
            return false
        }

        /// Draws the formatting over the plain text without changing it.
        func restyle(_ tv: UITextView) {
            textView = tv
            let base = Self.baseFont
            let storage = tv.textStorage
            let whole = NSRange(location: 0, length: storage.length)
            let mono = UIFont.monospacedSystemFont(ofSize: base.pointSize * 0.9, weight: .regular)
            storage.beginEditing()
            storage.setAttributes([.font: base, .foregroundColor: UIColor.label], range: whole)
            for run in DocFormat.runs(tv.text) where NSMaxRange(run.range) <= storage.length && run.range.length > 0 {
                let r = run.range
                switch run.style {
                case .heading(let n):
                    let style: UIFont.TextStyle = n == 1 ? .title1 : n == 2 ? .title2 : .title3
                    storage.addAttribute(.font, value: Self.font(UIFont.preferredFont(forTextStyle: style), adding: .traitBold), range: r)
                case .bold:
                    storage.enumerateAttribute(.font, in: r) { f, sub, _ in
                        storage.addAttribute(.font, value: Self.font(f as? UIFont ?? base, adding: .traitBold), range: sub)
                    }
                case .italic:
                    storage.enumerateAttribute(.font, in: r) { f, sub, _ in
                        storage.addAttribute(.font, value: Self.font(f as? UIFont ?? base, adding: .traitItalic), range: sub)
                    }
                case .strike:
                    storage.addAttribute(.strikethroughStyle, value: NSUnderlineStyle.single.rawValue, range: r)
                case .done:
                    storage.addAttributes([.strikethroughStyle: NSUnderlineStyle.single.rawValue, .foregroundColor: UIColor.secondaryLabel], range: r)
                case .code, .codeBlock:
                    storage.addAttributes([.font: mono, .backgroundColor: UIColor.secondarySystemFill], range: r)
                case .link:
                    storage.addAttributes([.foregroundColor: UIColor.link, .underlineStyle: NSUnderlineStyle.single.rawValue], range: r)
                case .quote:
                    storage.addAttribute(.foregroundColor, value: UIColor.secondaryLabel, range: r)
                case .marker:
                    storage.addAttribute(.foregroundColor, value: UIColor.tertiaryLabel, range: r)
                }
            }
            storage.endEditing()
            tv.typingAttributes = [.font: base, .foregroundColor: UIColor.label]
        }

        static func font(_ f: UIFont, adding trait: UIFontDescriptor.SymbolicTraits) -> UIFont {
            guard let d = f.fontDescriptor.withSymbolicTraits(f.fontDescriptor.symbolicTraits.union(trait)) else { return f }
            return UIFont(descriptor: d, size: f.pointSize)
        }
    }
}
