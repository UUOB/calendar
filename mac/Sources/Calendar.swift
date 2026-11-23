// Calendar for the Mac: SwiftUI over Calendar's core (core/, through UniFFI)
// and UUOB's SDK. What the screen shows comes from the core (`screen`, the
// same answers the Linux and Android windows get); the connect screen,
// Settings, the time field and the controls are UUOB's kit for apps
// (uuob/kit/mac). The screens themselves are in Views.swift and Sheets.swift.

import AppKit
import SwiftUI
import UniformTypeIdentifiers

let calendarApp = AppInfo(
    id: "calendar", name: "Calendar", noun: "events", symbol: "calendar", color: "red",
    version: version(),
    updateFeed: "https://raw.githubusercontent.com/jachymri/uuob-releases/main/calendar.json",
    updateKey: "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDU5QjY3MTNBQjA5NTI0QzAKUldUQUpKV3dPbkcyV2VzZU9XSWJQT29NWTR3angxNjU1VWJZazJsMi80Zjl6WDJoSWxMK0t3MlUK"
)

/// Height of an hour in the grid.
let HOUR: CGFloat = 52
/// The views the switch on top can offer.
/// The views Settings offers here.
let VIEWS: [(String, String)] = [("1d", "1 day"), ("2d", "2 days"), ("3d", "3 days"), ("4d", "4 days"), ("5d", "5 days"), ("6d", "6 days"), ("1w", "Week"), ("month", "Month"), ("year", "Year")]

@main
struct CalendarMacApp: App {
    @State private var model: CalendarModel
    @State private var updater = Updater(calendarApp)

    init() {
        _model = State(initialValue: CalendarModel(Uuob(calendarApp)))
    }

    var body: some Scene {
        WindowGroup("Calendar") {
            Gate(uuob: model.uuob, updater: updater) { CalendarView(model: model, updater: updater) }
                // Small windows work too: the calendars fold away and fewer days show.
                .frame(minWidth: 380, minHeight: 420)
                .background(Look.bg)
        }
        // No title bar: the sidebar and the calendar are the window, as on the web.
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1200, height: 800)
        .commands { Self.commands(model) }
    }

    /// Every shortcut, for the list (⌘?).
    @MainActor static func shortcutRows(_ model: CalendarModel) -> [(String, String)] { commands(model).all }

    /// The shared shortcuts (docs/design.md) and Calendar's own.
    @MainActor static func commands(_ model: CalendarModel) -> UuobCommands {
        let views = model.screen.views.array
        var extra: [Shortcut] = [
            Shortcut(title: "Today", key: "t") { model.cursor = today() },
            Shortcut(title: "Previous", key: .leftArrow) { model.move(-1) },
            Shortcut(title: "Next", key: .rightArrow) { model.move(1) },
            Shortcut(title: "Import a File…", key: "o") { model.pickFiles() },
        ]
        for (i, v) in views.prefix(4).enumerated() {
            extra.append(Shortcut(title: v.name.string, key: KeyEquivalent(Character("\(i + 1)"))) { model.show(v.id) })
        }
        return UuobCommands(
            settings: { model.settingsOpen = true },
            new: { model.newEventOn(nil) }, newTitle: "New Event",
            refresh: { Task { await model.reload() } },
            extraMenu: "Go", extra: extra
        )
    }
}

func nowMs() -> Int64 { Int64(Date().timeIntervalSince1970 * 1000) }

// The core's answers under names apart from the model's own.
private func coreScreen(_ state: String) -> String { screen(state: state) }
private func coreCard(_ state: String, _ key: String, _ from: String, _ to: String) -> String { card(state: state, key: key, from: from, to: to) }
func agoText(_ ms: Int64) -> String { ago(ms: ms) }

/// "HH:MM" of minutes since midnight.
func hhmm(_ m: Int) -> String { String(format: "%02d:%02d", min(m, 1439) / 60, min(m, 1439) % 60) }

/// Calendar's state: every event, calendar and preference, kept live from
/// UUOB. This reads, writes and remembers what the user is looking at.
@Observable
@MainActor
final class CalendarModel {
    let uuob: Uuob
    let settings: UuobSettings
    var data: [String: Any] = [:]
    var tasks = "[]"
    var view = ""
    var cursor = today()
    var narrow = false
    /// How many days fit side by side in the window (a narrow window shows fewer than the view has).
    var fit: Int?
    /// The calendars on the left, opened by hand in a narrow window.
    var sidebarOpen = false
    var hidden: [String]?
    var showTasks: Bool?
    var views: Any?
    var now = nowMs()
    var eventsKnown = false
    var prefsKnown = false
    var note = ""
    // What is open.
    var openCard: J?
    var editor: J?
    var scopeAsk: String?
    var calSheet: J?
    var settingsOpen = false
    var subscribing = false
    var importItems: [J]?
    var importResult: J?
    var busy = ""
    private var refreshedOnce = false
    private var upcomingGen = 0
    private var reloadGen = 0

    init(_ uuob: Uuob) {
        self.uuob = uuob
        settings = UuobSettings(uuob)
        let c = uuob.client
        // What was seen last, at once; then UUOB's.
        data = [
            "events": J(text: c.cachedAll(collection: "events")).value ?? [Any](),
            "infos": J(text: c.cachedAll(collection: "calendar")).value ?? [Any](),
            "containers": J(text: c.cachedContainers() ?? "[]").value ?? [Any](),
            "first": J(text: c.cached(collection: "calendars", container: nil) ?? "[]").array.first { $0.id == "personal" }?.value ?? NSNull(),
            "prefs": J(text: c.cached(collection: "prefs", container: nil) ?? "[]").value ?? [Any](),
            "answers": J(text: c.cached(collection: "answers", container: nil) ?? "[]").value ?? [Any](),
            "alerts": J(text: c.cached(collection: "alerts", container: nil) ?? "[]").value ?? [Any](),
            "subs": J(text: c.cached(collection: "subscriptions", container: nil) ?? "[]").value ?? [Any](),
        ]
        Task { try? await moveOldCalendars(uuob: c) }
        Task {
            await reload()
            for await e in uuob.events() where e.type.string == "changed" || e.state.string == "connected" {
                // Many changes at once are read once.
                reloadGen += 1
                let gen = reloadGen
                try? await Task.sleep(for: .milliseconds(150))
                if gen == reloadGen { await reload() }
            }
        }
        // The line for now moves on, and a window left open overnight rolls over to the new day.
        Task {
            var shown = today()
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(30))
                now = nowMs()
                let t = today()
                if t != shown {
                    if cursor == shown { cursor = t }
                    shown = t
                    scheduleUpcoming()
                }
            }
        }
        // Every hour the coming-up window moves on.
        Task {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(3600))
                scheduleUpcoming()
            }
        }
    }

    var client: UuobClient { uuob.client }
    private var main: String { J(data["containers"]).array.first { $0.main.bool }?.id ?? "calendar" }
    var events: String { J(data["events"] ?? [Any]()).text }

    func state() -> String {
        var st = data
        st["view"] = view
        st["cursor"] = cursor
        st["narrow"] = narrow
        st["year_ok"] = true
        if let fit { st["fit"] = fit }
        st["sunday_first"] = settings.sundayFirst
        st["hour12"] = settings.hour12
        st["now"] = now
        st["tasks"] = J(text: tasks).value ?? [Any]()
        if let hidden { st["hidden"] = hidden }
        if let showTasks { st["show_tasks"] = showTasks }
        if let views { st["views"] = views }
        return J(st).text
    }

    var screen: J { J(text: coreScreen(state())) }
    private func range() -> (String, String) {
        let d = screen.days.array.map(\.string)
        return (d.first ?? cursor, d.last ?? cursor)
    }
    var emails: String { screen.emails.text }

    func reload() async {
        var d = data
        if let cs = try? await client.containers() { d["containers"] = J(text: cs).value ?? [Any]() }
        let all = J(text: await client.all(collection: "events"))
        d["events"] = all.records.value ?? [Any]()
        d["infos"] = J(text: await client.all(collection: "calendar")).records.value ?? [Any]()
        if let f = try? await client.load(collection: "calendars", container: nil), let first = J(text: f).array.first(where: { $0.id == "personal" }) {
            d["first"] = first.value
        }
        var prefsRead = false
        for (k, c) in [("prefs", "prefs"), ("answers", "answers"), ("alerts", "alerts"), ("subs", "subscriptions")] {
            if let r = try? await client.load(collection: c, container: nil) {
                d[k] = J(text: r).value ?? [Any]()
                if k == "prefs" { prefsRead = true }
            }
        }
        data = d
        if all.complete.bool { eventsKnown = true }
        if prefsRead {
            let first = !prefsKnown
            prefsKnown = true
            hidden = nil
            views = nil
            if first && J(text: prefsOf(prefs: J(d["prefs"]).text)).show_tasks.bool {
                if let t = try? await client.load(collection: "tasks", container: "tasks") { tasks = t }
            }
        }
        scheduleUpcoming()
        // Calendar links are read again when the app opens, at most every few hours.
        if !refreshedOnce && prefsRead {
            refreshedOnce = true
            for sub in J(text: dueSubscriptions(subs: J(d["subs"]).text, containers: J(d["containers"]).text)).array {
                _ = try? await refresh(uuob: client, calendar: sub.id, url: sub.url.string, events: events, emails: emails)
            }
        }
    }

    /// What is coming up is written 1.5 s after the last change, once everything has been read.
    func scheduleUpcoming() {
        guard eventsKnown, prefsKnown else { return }
        upcomingGen += 1
        let gen = upcomingGen
        Task {
            try? await Task.sleep(for: .milliseconds(1500))
            guard gen == upcomingGen else { return }
            await client.upcoming(list: upcomingList(state: state()))
        }
    }

    /// Runs a write; a failure shows as a note.
    func run(_ work: @escaping @MainActor () async throws -> Void) {
        Task {
            do { try await work() } catch { note = error.userMessage }
        }
    }

    func show(_ v: String) { view = v }

    func move(_ dir: Int64) {
        // As many days as are shown, when the window shows fewer than the view has.
        let shown = screen.fitted.isNull ? screen.view.string : "\(screen.fitted.int)d"
        cursor = moveCursor(view: shown, cursor: cursor, dir: dir, sundayFirst: settings.sundayFirst)
    }

    func day(_ d: String) {
        let dayView = screen.dayView.string
        cursor = d
        view = dayView
    }

    func open(_ key: String) {
        let (from, to) = range()
        // The card remembers which occurrence it is, for Edit and your answer.
        guard var c = J(text: coreCard(state(), key, from, to)).value as? [String: Any], !c.isEmpty else { openCard = nil; return }
        c["key"] = key
        openCard = J(c)
    }

    func edit(_ key: String) {
        let (from, to) = range()
        openCard = nil
        editor = J(text: editEvent(state: state(), key: key, from: from, to: to))
    }

    func newEventOn(_ date: String?, start: String? = nil, end: String? = nil) {
        let s = screen
        let days = s.days.array.map(\.string)
        let t = today()
        let d = date ?? (s.view.string == "month" || days.contains(t) ? t : days.first ?? t)
        editor = J(text: newEvent(state: state(), date: d, start: start, end: end))
    }

    func save(_ ed: J, scope: String?) {
        let series = !ed.series.isNull
        if scope == nil && series && asksScope(editor: ed.text) {
            editor = ed
            scopeAsk = "save"
            return
        }
        editor = nil
        scopeAsk = nil
        let sc = scope ?? (series ? "all" : nil)
        let (evs, m) = (events, main)
        run { try await saveEvent(uuob: self.client, editor: ed.text, scope: sc, events: evs, main: m) }
    }

    func delete(_ ed: J, scope: String?) {
        if scope == nil && !ed.series.isNull {
            editor = ed
            scopeAsk = "delete"
            return
        }
        editor = nil
        scopeAsk = nil
        let (evs, m) = (events, main)
        run { try await deleteEvent(uuob: self.client, editor: ed.text, scope: scope, events: evs, main: m) }
    }

    private func replace(_ list: String, _ id: String, _ value: [String: Any]?) {
        var items = J(data[list] ?? [Any]()).array.filter { $0.id != id }.map { $0.value as Any }
        if var v = value {
            v["id"] = id
            items.append(v)
        }
        data[list] = items
    }

    func answer(_ key: String, _ a: String) {
        replace("answers", key, ["answer": a])
        if var c = openCard?.value as? [String: Any] {
            c["answer"] = a
            openCard = J(c)
        }
        scheduleUpcoming()
        run { try await setAnswer(uuob: self.client, key: key, answer: a) }
    }

    func ownAlert(_ key: String, _ choice: String) {
        replace("alerts", key, choice == "default" ? nil : ["before": choice == "none" ? false as Any : (Int64(choice) ?? 0) as Any])
        scheduleUpcoming()
        run { try await setOwnAlert(uuob: self.client, key: key, choice: choice) }
    }

    func toggleCal(_ id: String) {
        let s = screen
        var h = s.hidden.array.map(\.string)
        if let i = h.firstIndex(of: id) { h.remove(at: i) } else { h.append(id) }
        hidden = h
        let show = s.showTasks.bool
        let text = J(h).text
        run { try await saveViewPrefs(uuob: self.client, hidden: text, showTasks: show) }
    }

    func toggleTasks() {
        let s = screen
        let on = !s.showTasks.bool
        let h = s.hidden.text
        if !on {
            showTasks = false
            tasks = "[]"
            run { try await saveViewPrefs(uuob: self.client, hidden: h, showTasks: false) }
            return
        }
        // Showing tasks needs read access to the Tasks app's container.
        run {
            try await self.client.requestAccess(container: "tasks", access: "read")
            self.tasks = try await self.client.load(collection: "tasks", container: "tasks")
            self.showTasks = true
            try await saveViewPrefs(uuob: self.client, hidden: h, showTasks: true)
        }
    }

    func saveCal(_ sheet: J) {
        calSheet = nil
        let infos = J(data["infos"] ?? [Any]()).text, cals = screen.calendars.text
        run { _ = try await saveCalendar(uuob: self.client, sheet: sheet.text, infos: infos, calendars: cals) }
    }

    func deleteCal(_ id: String, subscribed: Bool) {
        calSheet = nil
        run { try await deleteCalendar(uuob: self.client, id: id, subscribed: subscribed) }
    }

    func share(_ id: String) {
        calSheet = nil
        if !client.remote() { uuob.open("share") }
        run { try await self.client.share(id: id) }
    }

    func refreshNow(_ id: String) {
        guard let sub = J(data["subs"] ?? [Any]()).array.first(where: { $0.id == id }) else { return }
        calSheet = nil
        note = "Updating…"
        let (evs, em) = (events, emails)
        run {
            let n = try await refresh(uuob: self.client, calendar: id, url: sub.url.string, events: evs, emails: em)
            self.note = "Updated: \(n) events."
        }
    }

    func subscribeTo(_ url: String, name: String) {
        subscribing = false
        note = "Reading the link…"
        let (n, em) = (UInt32(screen.calendars.array.count), emails)
        run {
            _ = try await subscribe(uuob: self.client, url: url, name: name, calendars: n, emails: em)
            self.note = "Subscribed."
        }
    }

    /// Import a file: the files chosen, read for the import sheet.
    func pickFiles() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = true
        panel.allowedContentTypes = [UTType(filenameExtension: "ics") ?? .data]
        panel.title = "Import a file"
        guard panel.runModal() == .OK else { return }
        importItems = panel.urls.map { u in
            if let text = try? String(contentsOf: u, encoding: .utf8) {
                var item = (J(text: readFile(name: u.lastPathComponent, text: text)).value as? [String: Any]) ?? [:]
                item["target"] = "new"
                return J(item)
            }
            return J(["name": u.lastPathComponent, "error": "This file could not be read as a calendar."])
        }
    }

    func runImport(_ items: [J]) {
        importItems = nil
        busy = "Importing…"
        let (n, em, text) = (UInt32(screen.calendars.array.count), emails, J(items.map { $0.value as Any }).text)
        Task {
            let r = await importFiles(uuob: client, items: text, calendars: n, emails: em)
            busy = ""
            importResult = J(text: r)
        }
    }

    /// Save as .ics file.
    func exportCal(_ id: String) {
        let name = screen.calendars.array.first { $0.id == id }?.name.str ?? "Calendar"
        let r = J(text: exportCalendar(events: events, container: id, name: name))
        let panel = NSSavePanel()
        panel.nameFieldStringValue = r.name.string
        panel.allowedContentTypes = [UTType(filenameExtension: "ics") ?? .data]
        guard panel.runModal() == .OK, let url = panel.url else { return }
        do {
            try r.text_.string.write(to: url, atomically: true, encoding: .utf8)
            note = "Saved \(r.count.int) events as \(url.lastPathComponent)."
        } catch {
            note = "The file could not be saved."
        }
    }

    /// The switch's views for this kind of screen, as a list (kept in order by the core).
    func setViews(_ list: [String]) {
        let key = narrow ? "narrow" : "wide"
        var v = (screen.switch_.value as? [String: Any]) ?? [:]
        let slot = J(v[key]).text
        v[key] = J(text: switchWithList(slot: slot, list: J(list).text)).value
        views = v
        let text = J(v).text
        run { try await saveSwitch(uuob: self.client, views: text) }
    }

    func setSwitch(_ change: [String: Any]) {
        let key = narrow ? "narrow" : "wide"
        var v = (screen.switch_.value as? [String: Any]) ?? [:]
        var slot = (v[key] as? [String: Any]) ?? [:]
        slot.merge(change) { _, b in b }
        v[key] = slot
        views = v
        let text = J(v).text
        run { try await saveSwitch(uuob: self.client, views: text) }
    }

    func setBefore(_ b: Int64?) {
        replace("prefs", "alerts", ["before": b.map { $0 as Any } ?? NSNull()])
        scheduleUpcoming()
        run { try await saveDefaultAlert(uuob: self.client, before: b) }
    }

    func forgetAddress(_ a: String) {
        let list = screen.emails.array.map(\.string).filter { $0 != a }
        replace("prefs", "me", ["emails": list])
        let text = J(list).text
        run { try await saveEmails(uuob: self.client, emails: text) }
    }
}

extension J {
    /// Ids for `ForEach`.
    var keyId: String { self["key"].string }
    var dateId: String { self["date"].string }
    var titleId: String { self["title"].string }
    /// Fields named like Swift words (`text`, `switch`).
    var text_: J { self["text"] }
    var switch_: J { self["switch"] }
}
