// Calendar's sheets, as Calendar.svelte drew them: the event card, the event
// sheet, "only this one or all of them?", the calendar sheet, Subscribe,
// Import and Settings, and the alert picker (AlertPick.svelte).

import AppKit
import SwiftUI

/// A field that opens a menu (the web version's `select.field`).
struct FieldMenu: View {
    let label: String
    let items: [(String, () -> Void)]
    var body: some View {
        Menu {
            ForEach(Array(items.enumerated()), id: \.offset) { _, it in Button(it.0, action: it.1) }
        } label: {
            HStack(spacing: 4) {
                Text(label).foregroundStyle(Look.text).lineLimit(1)
                Spacer(minLength: 4)
                Ico("chevron", size: 14).rotationEffect(.degrees(90)).foregroundStyle(Look.text2)
            }
            .font(.system(size: 17))
            .padding(.horizontal, 14).frame(maxWidth: .infinity, minHeight: 46)
            .background(Look.surface2, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(Look.line))
            .contentShape(Rectangle())
        }
        .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden)
    }
}

/// A date field ("YYYY-MM-DD" in and out).
struct DateField: View {
    let date: String
    let set: (String) -> Void
    var body: some View {
        let d = ISO8601DateFormatter.day.date(from: date) ?? Date()
        DatePicker("", selection: Binding(get: { d }, set: { set(ISO8601DateFormatter.day.string(from: $0)) }), displayedComponents: .date)
            .labelsHidden().datePickerStyle(.field).font(.system(size: 17))
            .padding(.horizontal, 10).frame(maxWidth: .infinity, minHeight: 46, alignment: .leading)
            .background(Look.surface2, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(Look.line))
    }
}

extension ISO8601DateFormatter {
    /// "YYYY-MM-DD" in this Mac's time zone.
    static let day: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withFullDate, .withDashSeparatorInDate]
        f.timeZone = .current
        return f
    }()
}

/// The alert picker: the usual times, or Custom…, which puts a time in its
/// place ("15:00" is 15 hours before, "0:30" half an hour, up to 168:00): Enter
/// sets it, Esc or a click elsewhere keeps what was there.
struct AlertPick: View {
    let options: [J]
    let current: String
    var plain = false
    let pick: (String) -> Void
    @State private var custom = false
    @State private var text = ""
    @FocusState private var focused: Bool

    private var before: String { beforeHhmm(minutes: Int64(current) ?? 15) }

    var body: some View {
        // A time chosen with Custom… shows by its name, though the list it came with doesn't have it.
        let shown = options.first { $0["value"].string == current }?.label.string ?? Int64(current).map { beforeLabel(minutes: $0) } ?? "Custom…"
        let items: [(String, () -> Void)] = options.map { o in (o.label.string, { pick(o["value"].string) }) } + [("Custom…", { text = before; custom = true })]
        if custom {
            HStack(spacing: 8) {
                TextField("h:mm", text: $text)
                    .textFieldStyle(UuobField()).multilineTextAlignment(.trailing).frame(width: 84)
                    .focused($focused)
                    .onSubmit(commit)
                    .onExitCommand { custom = false }
                    .onChange(of: focused) { if !focused { custom = false } }
                    .onAppear { DispatchQueue.main.async { focused = true } }
                    .uuobHint("Like 15:00 or 0:30 · Enter sets it, Esc cancels")
                Text("before").foregroundStyle(Look.text2).fixedSize()
            }
        } else if plain {
            // On the card: a plain line that opens the menu.
            Menu {
                ForEach(Array(items.enumerated()), id: \.offset) { _, it in Button(it.0, action: it.1) }
            } label: {
                HStack(spacing: 6) { Text(shown); Ico("chevron", size: 13).rotationEffect(.degrees(90)).foregroundStyle(Look.text2) }
                    .font(.system(size: 15)).foregroundStyle(Look.text).contentShape(Rectangle())
            }
            .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden).fixedSize()
        } else {
            FieldMenu(label: shown, items: items)
        }
    }

    /// A time sets it; anything else goes back to what was there.
    private func commit() {
        if let m = parseBefore(text: text) {
            custom = false
            pick(String(m))
        } else {
            text = before
        }
    }
}

// MARK: - The card

struct CardSheet: View {
    @Bindable var model: CalendarModel
    let c: J
    @Environment(\.dismiss) private var dismiss
    @State private var peopleOpen = false
    @State private var joiningOpen = false

    var body: some View {
        let key = c.key.string
        let akey = c.answerKey.string
        DetailSheet(title: c.title.string, onClose: { dismiss() }) {
            if c.canEdit.bool { IconButton("edit", size: 19, help: "Edit (⌘E)") { dismiss(); model.edit(key) }.keyboardShortcut("e") }
        } content: {
            VStack(alignment: .leading, spacing: 0) {
                Text(c.when.string).font(.system(size: 16, weight: .medium)).padding(.top, 10).padding(.bottom, 2)
                if let r = c["repeat"].str { Text(r).font(.system(size: 13)).foregroundStyle(Look.text2).padding(.top, 2) }
                HStack(spacing: 7) {
                    Circle().fill(Palette.color(c.color.string)).frame(width: 8, height: 8)
                    Text(c.calendar.string)
                }
                .font(.system(size: 13)).foregroundStyle(Look.text2).padding(.top, 2)
                if !c.meeting.isNull {
                    let m = c.meeting
                    Button { open(m.url.string) } label: {
                        HStack(spacing: 10) {
                            Ico("video", size: 20, stroke: 1.8)
                            Text("Join \(m.name.string)")
                            Spacer()
                            Text(m.host.string).font(.system(size: 13, weight: .medium)).opacity(0.8)
                        }
                        .font(.system(size: 16, weight: .semibold)).foregroundStyle(.white)
                        .padding(.horizontal, 16).frame(height: 50)
                        .background(Look.accent, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                    }
                    .buttonStyle(PressStyle()).padding(.top, 18)
                }
                VStack(alignment: .leading, spacing: 0) {
                    if !c.place.string.isEmpty { row("place") { Text(c.place.string) } }
                    if let u = c.url.str {
                        Button { open(u) } label: { row("link") { Text(c.url_shown.string).foregroundStyle(Look.accent) } }.buttonStyle(.plain)
                    }
                    if c.timed.bool {
                        row("bell") {
                            AlertPick(options: c.alertOptions.array, current: c.alert.string, plain: true) { model.ownAlert(akey, $0) }
                        }
                    }
                    let people = c.people.array
                    if !people.isEmpty {
                        Button { withAnimation(.easeOut(duration: 0.18)) { peopleOpen.toggle() } } label: {
                            row("people") {
                                HStack {
                                    Text(c.peopleSummary.string)
                                    Spacer()
                                    Ico("chevron", size: 16).rotationEffect(.degrees(peopleOpen ? 90 : 0)).foregroundStyle(Look.text2)
                                }
                            }
                        }
                        .buttonStyle(.plain)
                        if peopleOpen {
                            VStack(alignment: .leading, spacing: 6) {
                                ForEach(Array(people.enumerated()), id: \.offset) { _, p in person(p) }
                            }
                            .padding(.vertical, 10)
                        }
                    }
                    if c.hasNotes.bool { Text(linked(c.notes)).font(.system(size: 15)).textSelection(.enabled).padding(.vertical, 12) }
                    if c.hasJoining.bool {
                        Button { withAnimation(.easeOut(duration: 0.18)) { joiningOpen.toggle() } } label: {
                            HStack(spacing: 6) {
                                Text("Joining details")
                                Ico("chevron", size: 15).rotationEffect(.degrees(joiningOpen ? 90 : 0))
                            }
                            .font(.system(size: 13)).foregroundStyle(Look.text2).padding(.vertical, 8).contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        if joiningOpen { Text(linked(c.joining)).font(.system(size: 13)).foregroundStyle(Look.text2).textSelection(.enabled) }
                    }
                }
                .padding(.top, 10)
                if c.asksAnswer.bool {
                    VStack(spacing: 8) {
                        Segmented([("yes", "Going"), ("maybe", "Maybe"), ("no", "No")], selected: c.answer.string) { model.answer(akey, $0) }
                    }
                    .frame(maxWidth: .infinity).padding(.top, 16)
                }
            }
        }
    }

    /// `.row`: an icon and a line, a hairline under it.
    private func row<C: View>(_ icon: String, @ViewBuilder _ content: () -> C) -> some View {
        HStack(spacing: 12) {
            Ico(icon, size: 20, stroke: 1.8).foregroundStyle(Look.text2)
            content()
        }
        .font(.system(size: 15))
        .frame(maxWidth: .infinity, minHeight: 46, alignment: .leading)
        .padding(.vertical, 2)
        .overlay(alignment: .bottom) { Rectangle().fill(Look.line).frame(height: 1) }
        .contentShape(Rectangle())
    }

    private func person(_ p: J) -> some View {
        let mark: String
        switch p.answer.string {
        case "yes": mark = "✓"
        case "maybe": mark = "?"
        case "no": mark = "✕"
        default: mark = ""
        }
        return HStack(spacing: 8) {
            Text(mark).font(.system(size: 13, weight: .bold)).frame(width: 18)
                .foregroundStyle(p.answer.string == "yes" ? Look.ok : p.answer.string == "no" ? Look.danger : Look.text2)
            Text(p.name.str.flatMap { $0.isEmpty ? nil : $0 } ?? p.email.string).lineLimit(1)
            Spacer()
            if p.organizer.bool { Text("organizer").font(.system(size: 13)).foregroundStyle(Look.text2) }
            if p.you.bool { Text("you").font(.system(size: 13)).foregroundStyle(Look.text2) }
        }
        .font(.system(size: 15))
    }

    private func open(_ url: String) {
        if let u = URL(string: url) { NSWorkspace.shared.open(u) }
    }
}

/// Text with links that open in the browser, from the core's `[{text, href?}]`.
func linked(_ parts: J) -> AttributedString {
    var out = AttributedString()
    for p in parts.array {
        var piece = AttributedString(p.text_.string)
        // Links show in the accent colour by themselves.
        if let h = p.href.str, let u = URL(string: h) { piece.link = u }
        out += piece
    }
    return out
}

// MARK: - The event sheet

struct EventSheet: View {
    @Bindable var model: CalendarModel
    let start: J
    @State private var ed: [String: Any]

    init(model: CalendarModel, start: J) {
        self.model = model
        self.start = start
        _ed = State(initialValue: (start.value as? [String: Any]) ?? [:])
    }

    private var e: J { J(ed) }
    private var rep: [String: Any] { (ed["repeat"] as? [String: Any]) ?? [:] }
    private func setRep(_ k: String, _ v: Any) {
        var r = rep
        r[k] = v
        ed["repeat"] = r
    }
    private func text(_ k: String) -> Binding<String> { Binding(get: { (ed[k] as? String) ?? "" }, set: { ed[k] = $0 }) }

    var body: some View {
        let s = model.screen
        let isNew = e.id.isEmpty
        let allDay = e.all_day.bool
        let freq = e["repeat"].freq.str ?? "NONE"
        let cals = s.calendars.array.filter { $0.canEdit.bool || $0.id == e.calendar.string }
        let cal = cals.first { $0.id == e.calendar.string }
        EditSheet(placeholder: "New event", title: text("title"), isNew: isNew,
                  onCancel: { model.editor = nil }, onSave: { model.save(e, scope: nil) }, onDelete: { model.delete(e, scope: nil) }) {
            // As on the card: the calendar under the title.
            HStack(spacing: 7) {
                Circle().fill(Palette.color(cal?.color.string ?? "blue")).frame(width: 8, height: 8)
                PlainMenu(label: cal?.name.string ?? "", items: cals.map { c in (c.name.string, { ed["calendar"] = c.id }) }, color: Look.text2)
            }
            .font(.system(size: 13))
        } content: {
            VStack(alignment: .leading, spacing: 0) {
                IconRow("clock") {
                    Text("All-day")
                    Spacer()
                    Toggle("", isOn: Binding(get: { allDay }, set: { ed["all_day"] = $0 })).labelsHidden()
                }
                ForEach([("Starts", "date", "start"), ("Ends", "end_date", "end")], id: \.0) { label, dk, tk in
                    IconRow(nil) {
                        Text(label).foregroundStyle(Look.text2)
                        Spacer()
                        DatePicker("", selection: dayBinding(dk), displayedComponents: .date).labelsHidden().datePickerStyle(.field).fixedSize()
                        if !allDay {
                            TimeField(value: Binding(get: { e[tk].str }, set: { if let t = $0 { ed[tk] = t } }), hour12: model.settings.hour12)
                                .textFieldStyle(.plain).multilineTextAlignment(.trailing).frame(width: 72)
                        }
                    }
                }
                IconRow("repeat") {
                    let freqs: [(String, String)] = [("NONE", "Never"), ("DAILY", "Every day"), ("WEEKLY", "Every week"), ("MONTHLY", "Every month"), ("YEARLY", "Every year")]
                        + (e["repeat"].raw.str != nil ? [("CUSTOM", e["repeat"].label.string)] : [])
                    PlainMenu(label: freqs.first { $0.0 == freq }?.1 ?? "Never", items: freqs.map { f in (f.1, { setRep("freq", f.0) }) })
                    Spacer()
                }
                if freq == "WEEKLY" { weekdays.padding(.vertical, 10).overlay(alignment: .bottom) { Rectangle().fill(Look.line).frame(height: 1) } }
                if freq != "NONE" && freq != "CUSTOM" { repeatDetails(freq) }
                if !allDay {
                    let current = e.alert.string
                    IconRow("bell") {
                        AlertPick(options: J(text: alertOptions(current: current, defaultBefore: s.before.isNull ? nil : s.before.int64, withDefault: true)).array, current: current, plain: true) { ed["alert"] = $0 }
                        Spacer()
                    }
                }
                IconRow("place") {
                    TextField("Location", text: text("location")).textFieldStyle(.plain)
                }
                IconRow("notes", last: true) {
                    TextField("Notes", text: text("notes"), axis: .vertical).textFieldStyle(.plain).lineLimit(1...8).padding(.vertical, 10)
                }
            }
        }
        // "Only this one, or all of them?" over the sheet, as on the web.
        .sheet(isPresented: Binding(get: { model.scopeAsk != nil }, set: { if !$0 { model.scopeAsk = nil } })) { ScopeSheet(model: model) }
    }

    /// A date in the editor as a picker's Date; a new start moves the end with it.
    private func dayBinding(_ dk: String) -> Binding<Date> {
        Binding(
            get: { ISO8601DateFormatter.day.date(from: e[dk].string) ?? Date() },
            set: { v in
                let d = ISO8601DateFormatter.day.string(from: v)
                if dk == "date" {
                    ed = (J(text: moveStart(editor: e.text, date: d)).value as? [String: Any]) ?? ed
                } else {
                    ed["end_date"] = d
                }
            }
        )
    }

    /// `.weekdays`: a round button per day; at least one stays on.
    private var weekdays: some View {
        let days = e["repeat"].days.array.map(\.string)
        return HStack(spacing: 6) {
            ForEach([("MO", "M"), ("TU", "T"), ("WE", "W"), ("TH", "T"), ("FR", "F"), ("SA", "S"), ("SU", "S")], id: \.0) { code, letter in
                let on = days.contains(code)
                Button {
                    var list = days
                    if on { if list.count > 1 { list.removeAll { $0 == code } } } else { list.append(code) }
                    setRep("days", list)
                } label: {
                    Text(letter).font(.system(size: 15, weight: .semibold)).foregroundStyle(on ? .white : Look.text2)
                        .frame(width: 38, height: 38).background(on ? Look.accent : Look.surface2, in: Circle())
                }
                .buttonStyle(PressStyle()).uuobHint(code)
            }
        }
        .frame(maxWidth: .infinity)
    }

    /// Every N days/weeks…, until forever, a date or a number of times: one quiet row.
    @ViewBuilder private func repeatDetails(_ freq: String) -> some View {
        let r = e["repeat"]
        let unit = ["DAILY": "days", "WEEKLY": "weeks", "MONTHLY": "months"][freq] ?? "years"
        let ends = r.ends.str ?? "never"
        let choices = [("never", "forever"), ("until", "until a date"), ("count", "a number of times")]
        IconRow(nil) {
            Text("Every").foregroundStyle(Look.text2)
            TextField("", value: Binding(get: { max(1, r.interval.int) }, set: { setRep("interval", min(99, max(1, $0))) }), format: .number)
                .textFieldStyle(.plain).multilineTextAlignment(.center).frame(width: 34)
                .padding(.vertical, 3).background(Look.surface2, in: RoundedRectangle(cornerRadius: 7))
            Text(unit).foregroundStyle(Look.text2)
            Spacer()
            PlainMenu(label: choices.first { $0.0 == ends }?.1 ?? "forever", items: choices.map { c in (c.1, { setRep("ends", c.0) }) })
        }
        if ends == "until" {
            IconRow(nil) {
                Text("Until").foregroundStyle(Look.text2)
                Spacer()
                DatePicker("", selection: Binding(get: { ISO8601DateFormatter.day.date(from: r.until.str ?? e.date.string) ?? Date() },
                                                  set: { setRep("until", ISO8601DateFormatter.day.string(from: $0)) }),
                           displayedComponents: .date).labelsHidden().datePickerStyle(.field).fixedSize()
            }
        } else if ends == "count" {
            IconRow(nil) {
                TextField("", value: Binding(get: { max(1, r.count.int == 0 ? 10 : r.count.int) }, set: { setRep("count", min(999, max(1, $0))) }), format: .number)
                    .textFieldStyle(.plain).multilineTextAlignment(.center).frame(width: 44)
                    .padding(.vertical, 3).background(Look.surface2, in: RoundedRectangle(cornerRadius: 7))
                Text("times").foregroundStyle(Look.text2)
                Spacer()
            }
        }
    }
}

/// "Change (or delete) only this one, or all of them?" for a repeating event.
struct ScopeSheet: View {
    @Bindable var model: CalendarModel
    var body: some View {
        let delete = model.scopeAsk == "delete"
        let verb = delete ? "Delete" : "Change"
        let ed = model.editor ?? J(nil)
        ChoiceSheet(
            title: "\(verb) a repeating event",
            message: "\(verb) only this one, on \(friendlyDate(date: ed.occurrence.string)), or every time it repeats?",
            choices: [
                ("Only this one", delete ? .danger : .plain, { delete ? model.delete(ed, scope: "this") : model.save(ed, scope: "this") }),
                ("All of them", delete ? .danger : .primary, { delete ? model.delete(ed, scope: "all") : model.save(ed, scope: "all") }),
            ]
        )
    }
}

// MARK: - Calendars

/// New calendar / edit one / about one shared with you.
struct CalSheet: View {
    @Bindable var model: CalendarModel
    let start: J
    @State private var name = ""
    @State private var color = "blue"

    var body: some View {
        let id = start.id.isEmpty ? nil : start.id
        Group {
            if !start.from.isNull, let id {
                // Shared with you: what it is, and Leave.
                DetailSheet(title: start.name.string, onClose: { model.calSheet = nil }, actions: { EmptyView() }) {
                    Text("Shared by \(start.from.name.string)" + (start.canWrite.bool ? "" : " · view only"))
                        .font(.system(size: 13)).foregroundStyle(Look.text2).padding(.top, 6).padding(.bottom, 8)
                    rows(id)
                } footer: {
                    HStack {
                        Button("Leave calendar", role: .destructive) { model.deleteCal(id, subscribed: false) }.buttonStyle(UuobButton(.danger, small: true))
                        Spacer()
                    }
                    .padding(.top, 18)
                }
            } else {
                EditSheet(placeholder: "Name, e.g. Work", title: $name, isNew: id == nil,
                          canSave: !name.trimmingCharacters(in: .whitespaces).isEmpty,
                          onCancel: { model.calSheet = nil },
                          onSave: {
                              var out = (start.value as? [String: Any]) ?? [:]
                              out["name"] = name
                              out["color"] = color
                              model.saveCal(J(out))
                          },
                          onDelete: id == nil || start.main.bool ? nil : { model.deleteCal(id!, subscribed: start.subscribed.bool) }) {
                    // Its colour, under the name.
                    HStack(spacing: 10) {
                        ForEach(Palette.names, id: \.self) { c in
                            Button { color = c } label: {
                                Circle().fill(Palette.color(c)).frame(width: 22, height: 22)
                                    .overlay(Circle().strokeBorder(Look.text, lineWidth: c == color ? 2 : 0).padding(-3))
                            }
                            .buttonStyle(PressStyle()).uuobHint(c)
                        }
                    }
                    .padding(.vertical, 4)
                } content: {
                    if let id { rows(id) }
                }
            }
        }
        .onAppear {
            name = start.name.string
            color = start.color.str ?? "blue"
        }
    }

    /// Follows a link, save as a file, share: one icon row each.
    @ViewBuilder private func rows(_ id: String) -> some View {
        let sub = start.subscribed.bool ? J(model.data["subs"] ?? [Any]()).array.first { $0.id == start.id } : nil
        VStack(spacing: 0) {
            if let sub {
                IconRow("link") {
                    Text("Updated \(agoText(sub.refreshed.int64))").foregroundStyle(Look.text2)
                    Spacer()
                    Button("Update now") { model.refreshNow(id) }.buttonStyle(UuobButton(.ghost, small: true))
                }
            }
            Button { model.exportCal(id) } label: { IconRow("download", last: start.main.bool || !start.from.isNull) { Text("Save as .ics file"); Spacer() } }.buttonStyle(.plain)
            if !start.main.bool && start.from.isNull {
                Button { model.share(id) } label: {
                    IconRow("share", last: true) { Text(start.sharedWith.array.isEmpty ? "Share calendar…" : start.note.string).foregroundStyle(Look.accent); Spacer() }
                }
                .buttonStyle(.plain)
            }
        }
    }
}

struct SubscribeSheet: View {
    @Bindable var model: CalendarModel
    @State private var url = ""
    @State private var name = ""
    var body: some View {
        let ok = !url.trimmingCharacters(in: .whitespaces).isEmpty
        EditSheet(placeholder: "Name (optional)", title: $name, isNew: true, canSave: ok, saveLabel: "Subscribe",
                  onCancel: { model.subscribing = false }, onSave: { model.subscribeTo(url, name: name) }) {
            IconRow("link", last: true) {
                TextField(".ics or webcal:// link", text: $url).textFieldStyle(.plain).onSubmit { if ok { model.subscribeTo(url, name: name) } }
            }
        }
    }
}

/// The files chosen for Import: what each holds, and where it goes.
struct ImportSheet: View {
    @Bindable var model: CalendarModel
    let items: [J]
    @State private var targets: [String] = []

    var body: some View {
        let cals = model.screen.calendars.array.filter { $0.canEdit.bool }
        DetailSheet(title: "Import", onClose: { model.importItems = nil }, actions: { EmptyView() }) {
            VStack(spacing: 0) {
                ForEach(Array(items.enumerated()), id: \.offset) { i, it in
                    IconRow("calendar", last: i == items.count - 1) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(it.name.string).lineLimit(1).truncationMode(.middle)
                            if let err = it.error.str {
                                Text(err).font(.system(size: 13)).foregroundStyle(Look.danger)
                            } else {
                                let rep = it.repeating.int
                                Text("\(it.events.int) events" + (rep > 0 ? ", \(rep) repeating" : "")).font(.system(size: 13)).foregroundStyle(Look.text2)
                            }
                        }
                        Spacer()
                        if it.error.isNull {
                            let target = i < targets.count ? targets[i] : "new"
                            PlainMenu(label: target == "new" ? "New calendar" : (cals.first { $0.id == target }?.name.string ?? ""),
                                      items: [("New calendar", { set(i, "new") })] + cals.map { c in (c.name.string, { set(i, c.id) }) })
                        }
                    }
                }
            }
            .padding(.top, 8)
        } footer: {
            HStack {
                Spacer()
                Button("Import") {
                    let out = items.enumerated().map { i, it -> J in
                        var v = (it.value as? [String: Any]) ?? [:]
                        v["target"] = i < targets.count ? targets[i] : "new"
                        return J(v)
                    }
                    model.runImport(out)
                }
                .buttonStyle(UuobButton(.primary, small: true)).keyboardShortcut(.return, modifiers: .command)
                .disabled(!items.contains { $0.error.isNull })
            }
            .padding(.top, 18)
        }
        .onAppear { targets = items.map { _ in "new" } }
    }

    private func set(_ i: Int, _ t: String) {
        while targets.count <= i { targets.append("new") }
        targets[i] = t
    }
}

/// What an import did.
struct ImportResult: View {
    let result: J
    var body: some View {
        let r = result
        var text = "Imported \(r.events.int) \(r.events.int == 1 ? "event" : "events")."
        if let e = r.error.str { text += " \(e) Importing the file again picks up where it stopped." }
        let n = r.unreadable.int
        if n > 0 { text += " \(n) \(n == 1 ? "event repeats" : "events repeat") in a way this calendar can't show yet; they appear only the first time." }
        let z = r.zones.array.map(\.string)
        if !z.isEmpty { text += " Times in \(z.joined(separator: ", ")) were read in the calendar's own time zone." }
        return ChoiceSheet(title: "Import", message: text + "\n\nImporting the same file again updates these events instead of adding them twice.", choices: [("Done", .primary, {})], cancel: false)
    }
}

// MARK: - Settings

struct CalendarSettings: View {
    @Bindable var model: CalendarModel
    let s: J

    var body: some View {
        let on = s.views.array.map(\.id)
        let before: Int64? = s.before.isNull ? nil : s.before.int64
        let current = before.map { String($0) } ?? "none"
        SettingsSheet(app: calendarApp, settings: model.settings, week: true) {
            RowsLabel("Calendar")
            // How a date is written in numbers (the title shows today's on a hover).
            IconRow("calendar", last: true) {
                Text("Date")
                Spacer(minLength: 8)
                Segmented(s.dateFormats.array.map { ($0.id, $0.name.string) }, selected: s.dateFormat.string) { model.setDateFormat($0) }
            }
            RowsLabel("Views")
            // Tick the views the switch on top offers; it shows them shortest first.
            ForEach(Array(VIEWS.enumerated()), id: \.offset) { i, v in
                let ticked = on.contains(v.0)
                // At most four: the switch stays small.
                let can = ticked ? on.count > 1 : on.count < 4
                Button {
                    if can { model.setViews(ticked ? on.filter { $0 != v.0 } : on + [v.0]) }
                } label: {
                    IconRow(i == 0 ? "calendar" : nil, last: i == VIEWS.count - 1) {
                        Text(v.1)
                        Spacer()
                        Check(on: ticked, size: 20).opacity(can || ticked ? 1 : 0.4)
                    }
                }
                .buttonStyle(.plain)
            }
            RowsLabel("Events")
            IconRow("bell") {
                Text("Default alert")
                Spacer(minLength: 8)
                AlertPick(options: J(text: alertOptions(current: current, defaultBefore: before, withDefault: false)).array, current: current, plain: true) { model.setBefore(Int64($0)) }
            }
            let emails = s.emails.array.map(\.string)
            IconRow("at", last: emails.isEmpty) {
                Text("Your addresses")
                Spacer()
                if emails.isEmpty { Text("None yet").foregroundStyle(Look.text3) }
            }
            ForEach(Array(emails.enumerated()), id: \.offset) { i, a in
                IconRow(nil, last: i == emails.count - 1) {
                    Text(a).foregroundStyle(Look.text2).lineLimit(1)
                    Spacer()
                    IconButton("close", size: 15, help: "Remove") { model.forgetAddress(a) }
                }
            }
        }
    }
}
