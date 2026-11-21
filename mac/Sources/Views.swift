// Calendar's window: the calendars on the left, the header, then the week
// (the hour grid) or the month, as Calendar.svelte laid them out.

import SwiftUI

struct CalendarView: View {
    @Bindable var model: CalendarModel
    @Bindable var updater: Updater

    var body: some View {
        let s = model.screen
        HStack(spacing: 0) {
            Sidebar(model: model, updater: updater, s: s).frame(width: 240)
            Rectangle().fill(Look.line).frame(width: 1)
            VStack(spacing: 0) {
                Header(model: model, s: s)
                if s.view.string == "month" { MonthView(model: model, s: s) } else { WeekView(model: model, s: s) }
            }
            .padding(.horizontal, 28).padding(.top, 26)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(Look.bg)
        }
        .ignoresSafeArea(.container, edges: .top)
        .foregroundStyle(Look.text)
        .overlay(alignment: .bottom) { Toast(text: model.busy.isEmpty ? model.note : model.busy) }
        .task(id: model.note) {
            // A note goes away by itself.
            guard !model.note.isEmpty else { return }
            try? await Task.sleep(for: .seconds(4))
            model.note = ""
        }
        .sheet(item: $model.openCard) { c in CardSheet(model: model, c: c) }
        // The editor keeps one identity while it is open, so "only this one or all?" can sit on top of it.
        .sheet(item: Binding(get: { model.editor.map { Keyed($0, id: "editor") } }, set: { if $0 == nil { model.editor = nil; model.scopeAsk = nil } })) { k in
            EventSheet(model: model, start: k.value)
        }
        .sheet(item: $model.calSheet) { c in CalSheet(model: model, start: c) }
        .sheet(isPresented: $model.subscribing) { SubscribeSheet(model: model) }
        .sheet(item: Binding(get: { model.importItems.map { Keyed(J($0.map { $0.value as Any }), id: "import") } }, set: { if $0 == nil { model.importItems = nil } })) { _ in
            ImportSheet(model: model, items: model.importItems ?? [])
        }
        .sheet(item: $model.importResult) { r in ImportResult(result: r) }
        .sheet(isPresented: $model.settingsOpen) { CalendarSettings(model: model, s: model.screen) }
    }
}

/// A value for `.sheet(item:)` with an identity of its own (a new event has no id yet).
struct Keyed: Identifiable {
    let value: J
    let id: String
    init(_ value: J, id: String) {
        self.value = value
        self.id = id
    }
}

/// The web version's toast: a small pill at the bottom.
struct Toast: View {
    let text: String
    var body: some View {
        if !text.isEmpty {
            Text(text).font(.system(size: 15, weight: .medium)).foregroundStyle(Look.text)
                .padding(.horizontal, 20).padding(.vertical, 12)
                .background(Look.elevated, in: Capsule())
                .overlay(Capsule().strokeBorder(Look.lineStrong))
                .shadow(color: .black.opacity(0.25), radius: 14, y: 8)
                .padding(.bottom, 28)
                .transition(.opacity)
        }
    }
}

/// A button that shrinks a little while pressed.
struct PressStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.scaleEffect(configuration.isPressed ? 0.96 : 1).animation(.easeOut(duration: 0.12), value: configuration.isPressed)
    }
}

// MARK: - Sidebar

struct Sidebar: View {
    @Bindable var model: CalendarModel
    @Bindable var updater: Updater
    let s: J

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                // Room for the window's buttons, and the update button when there is one.
                HStack { Spacer(); UpdateButton(updater: updater) }.frame(minHeight: 28).padding(.bottom, 4)
                HStack {
                    SectionLabel("Calendars")
                    Spacer()
                    Menu {
                        Button { model.calSheet = J(["name": "", "color": Palette.names[(s.calendars.array.count + 1) % Palette.names.count]]) } label: { Text("New calendar") }
                        Button { model.pickFiles() } label: { Text("Import a file") }
                        Button { model.subscribing = true } label: { Text("Subscribe to a link") }
                    } label: {
                        Ico("plus", size: 18, stroke: 2.2).foregroundStyle(Look.accent).frame(width: 30, height: 30).contentShape(Circle())
                    }
                    .menuStyle(.button).buttonStyle(IconButtonStyle()).menuIndicator(.hidden).fixedSize()
                    .help("Add a calendar")
                }
                .padding(.leading, 8).padding(.bottom, 4)
                ForEach(s.calendars.array) { c in CalRow(model: model, c: c) }
                SectionLabel("From other apps").padding(.top, 28).padding(.leading, 8).padding(.bottom, 4)
                HStack(spacing: 12) {
                    Button { model.toggleTasks() } label: { Check(on: s.showTasks.bool, ring: Look.text2) }.buttonStyle(PressStyle()).help("Show tasks")
                    Text("Tasks with dates").font(.system(size: 15, weight: .medium))
                    Spacer()
                    if !s.showTasks.bool { Ico("lock", size: 14).foregroundStyle(Look.text3) }
                }
                .padding(.horizontal, 8).padding(.vertical, 7)
                if !s.showTasks.bool {
                    Text("Asks to read your Tasks data.").font(.system(size: 13)).foregroundStyle(Look.text2).padding(.horizontal, 8).padding(.top, 4)
                }
            }
            .padding(.horizontal, 16).padding(.top, 20)
        }
        .safeAreaInset(edge: .bottom) {
            HStack {
                Button { model.settingsOpen = true } label: { IconLabel("settings", "Settings") }.buttonStyle(UuobButton(.quiet))
                Spacer()
            }
            .padding(.horizontal, 16).padding(.bottom, 18)
        }
        .background(Look.elevated)
    }
}

/// `.cal-row`: its check in the calendar's colour, the name, marks, and Edit on hover.
struct CalRow: View {
    @Bindable var model: CalendarModel
    let c: J
    @State private var hover = false
    var body: some View {
        HStack(spacing: 12) {
            Button { model.toggleCal(c.id) } label: { Check(on: !c.hidden.bool, ring: Palette.color(c.color.string)) }
                .buttonStyle(PressStyle()).help("Show \(c.name.string)")
            Text(c.name.string).font(.system(size: 15, weight: .medium)).lineLimit(1)
            Spacer(minLength: 0)
            if c.subscribed.bool { Ico("link", size: 15).foregroundStyle(Look.text3).help("Subscribed to a link") }
            if c.shared.bool { Ico("people", size: 15).foregroundStyle(Look.text3).help(c.note.string) }
            Button { model.calSheet = c } label: { Ico("edit", size: 15) }
                .buttonStyle(IconButtonStyle()).frame(width: 28, height: 28).opacity(hover ? 1 : 0)
                .help(c.from.isNull ? "Edit" : "About this calendar")
        }
        .padding(.horizontal, 8).padding(.vertical, 3)
        .background(hover ? Look.hover : .clear, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .onHover { hover = $0 }
    }
}

// MARK: - Header

struct Header: View {
    @Bindable var model: CalendarModel
    let s: J

    var body: some View {
        HStack(spacing: 14) {
            (Text(s.title.month.string).fontWeight(.bold) + Text(" \(s.title.year.text)").fontWeight(.regular).foregroundStyle(Look.text2))
                .font(.system(size: 34)).tracking(-0.85).lineLimit(1)
            Spacer(minLength: 0)
            Segmented(s.views.array.map { ($0.id, $0.name.string) }, selected: s.view.string) { model.show($0) }
            HStack(spacing: 2) {
                IconButton("left", size: 22, help: "Previous") { model.move(-1) }
                Button("Today") { model.cursor = today() }.buttonStyle(UuobButton(small: true))
                IconButton("right", size: 22, help: "Next") { model.move(1) }
            }
            Button { model.newEventOn(nil) } label: {
                Ico("plus", size: 20, stroke: 2.2).foregroundStyle(.white).frame(width: 36, height: 36).background(Look.accent, in: Circle())
            }
            .buttonStyle(PressStyle()).help("New event")
        }
        .padding(.bottom, 18)
    }
}

// MARK: - Week

struct WeekView: View {
    @Bindable var model: CalendarModel
    let s: J

    var body: some View {
        let cols = s.columns.array
        let dayView = s.dayView.string
        VStack(spacing: 0) {
            // The days.
            HStack(spacing: 0) {
                Color.clear.frame(width: 56, height: 1)
                ForEach(cols, id: \.dateId) { col in
                    let today = col.today.bool
                    Button {
                        if cols.count > 1 && dayView != s.view.string { model.day(col.date.string) }
                    } label: {
                        HStack(alignment: .firstTextBaseline, spacing: 6) {
                            Text(col.weekday.string.uppercased()).font(.system(size: 13, weight: .medium)).tracking(0.4)
                                .foregroundStyle(today ? Palette.color("red") : Look.text2)
                            Text("\(col.day.int)").font(.system(size: 20, weight: .semibold))
                                .foregroundStyle(today ? .white : Look.text)
                                .frame(width: 34, height: 34).background(today ? Palette.color("red") : .clear, in: Circle())
                        }
                        .padding(.top, 6).padding(.bottom, 10)
                        .frame(maxWidth: .infinity).contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }
            // The all-day row.
            HStack(alignment: .top, spacing: 0) {
                // The padding inside the 56 the hours take, so the columns line up.
                Text("all-day").font(.system(size: 11)).foregroundStyle(Look.text3).padding(.trailing, 8).padding(.top, 6).frame(width: 56, alignment: .trailing)
                ForEach(cols, id: \.dateId) { col in
                    VStack(alignment: .leading, spacing: 2) {
                        ForEach(col.allDay.array, id: \.keyId) { p in AllDayPill(model: model, p: p) }
                        ForEach(col.tasksAllDay.array, id: \.titleId) { t in TaskMark(title: t.title.string) }
                    }
                    .padding(.horizontal, 2).padding(.vertical, 3)
                    .frame(maxWidth: .infinity, minHeight: 30, alignment: .topLeading)
                    .overlay(alignment: .leading) { Rectangle().fill(Look.line).frame(width: 1) }
                }
            }
            Rectangle().fill(Look.lineStrong).frame(height: 1)
            // The hours.
            ScrollViewReader { proxy in
                ScrollView(.vertical) {
                    HStack(alignment: .top, spacing: 0) {
                        Hours(hour12: model.settings.hour12)
                        ForEach(cols, id: \.dateId) { col in
                            DayColumn(model: model, col: col, nowMinute: s.nowMinute.int)
                        }
                    }
                    .frame(height: HOUR * 24)
                }
                // A scroll bar would take width from the days, and they'd no longer line up with the heads above.
                .scrollIndicators(.never)
                .onAppear { proxy.scrollTo("h7", anchor: .top) }
            }
        }
    }
}

/// The hour labels on the left (each anchor is where "Today" scrolls to).
struct Hours: View {
    let hour12: Bool
    var body: some View {
        VStack(spacing: 0) {
            ForEach(0..<24, id: \.self) { h in
                ZStack(alignment: .topTrailing) {
                    Color.clear
                    if h > 0 {
                        Text(friendlyTime(time: hhmm(h * 60), hour12: hour12)).font(.system(size: 11)).foregroundStyle(Look.text3)
                            .lineLimit(1).fixedSize().offset(y: -7).padding(.trailing, 8)
                    }
                }
                .frame(height: HOUR).id("h\(h)")
            }
        }
        .frame(width: 56)
    }
}

/// One day of the grid: its lines, its events side by side, drag to make one, the line for now.
struct DayColumn: View {
    @Bindable var model: CalendarModel
    let col: J
    let nowMinute: Int
    @State private var drag: (from: Int, to: Int)?

    private func snap(_ y: CGFloat) -> Int { max(0, min(1425, Int((y / HOUR * 4).rounded()) * 15)) }

    var body: some View {
        GeometryReader { geo in
            let w = geo.size.width
            ZStack(alignment: .topLeading) {
                // The background takes the clicks and drags; the events above take their own.
                Rectangle().fill(col.today.bool ? Look.accent.opacity(0.03) : .clear).contentShape(Rectangle())
                    .gesture(
                        DragGesture(minimumDistance: 0)
                            .onChanged { v in
                                if drag == nil {
                                    let m = snap(v.startLocation.y)
                                    drag = (m, m + 60)
                                } else if abs(v.translation.height) > 4, let d = drag {
                                    drag = (d.from, max(d.from + 15, min(1440, snap(v.location.y))))
                                }
                            }
                            .onEnded { _ in
                                guard let d = drag else { return }
                                drag = nil
                                model.newEventOn(col.date.string, start: hhmm(d.from), end: hhmm(min(d.to, 1439)))
                            }
                    )
                Canvas { ctx, size in
                    var p = Path()
                    for h in 0..<24 {
                        p.move(to: CGPoint(x: 0, y: CGFloat(h) * HOUR + 0.5))
                        p.addLine(to: CGPoint(x: size.width, y: CGFloat(h) * HOUR + 0.5))
                    }
                    p.move(to: CGPoint(x: 0.5, y: 0))
                    p.addLine(to: CGPoint(x: 0.5, y: size.height))
                    ctx.stroke(p, with: .color(Look.line), lineWidth: 1)
                }
                .allowsHitTesting(false)
                ForEach(col.timed.array, id: \.keyId) { b in
                    let lanes = CGFloat(max(1, b.lanes.double)), lane = CGFloat(b.lane.double)
                    let h = max(22, CGFloat(b.end.double - b.start.double) / 60 * HOUR - 2)
                    let top = CGFloat(b.start.double) / 60 * HOUR + 1
                    Button { model.open(b.key.string) } label: { EventBlock(b: b, height: h) }
                        .buttonStyle(PressStyle())
                        .frame(width: max(8, w / lanes - 4), height: h)
                        .offset(x: lane / lanes * w + 2, y: top)
                }
                ForEach(col.tasksTimed.array, id: \.titleId) { t in
                    TaskMark(title: t.title.string).frame(width: max(8, w - 4), alignment: .leading)
                        .offset(x: 2, y: CGFloat(t.minute.double) / 60 * HOUR).allowsHitTesting(false)
                }
                if let d = drag {
                    let top = CGFloat(d.from) / 60 * HOUR, height = CGFloat(d.to - d.from) / 60 * HOUR
                    Text("\(friendlyTime(time: hhmm(d.from), hour12: model.settings.hour12)) – \(friendlyTime(time: hhmm(min(d.to, 1439)), hour12: model.settings.hour12))")
                        .font(.system(size: 12)).monospacedDigit().foregroundStyle(Palette.mix("blue", toward: Palette.text, 0.45))
                        .padding(.horizontal, 8).padding(.vertical, 3)
                        .frame(width: w - 4, height: height, alignment: .topLeading)
                        .background(Palette.mix("blue", toward: Palette.bg, 0.76), in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(Look.accent, style: StrokeStyle(lineWidth: 1, dash: [4, 3])))
                        .opacity(0.85)
                        .offset(x: 2, y: top).allowsHitTesting(false)
                }
                if col.today.bool {
                    let y = CGFloat(nowMinute) / 60 * HOUR
                    ZStack(alignment: .leading) {
                        Rectangle().fill(Palette.color("red")).frame(height: 2)
                        Circle().fill(Palette.color("red")).frame(width: 10, height: 10).offset(x: -5)
                    }
                    .frame(width: w).offset(y: y - 1).allowsHitTesting(false)
                }
            }
        }
        .frame(height: HOUR * 24)
    }
}

/// How strongly an event is tinted with its calendar's colour, by kind: a
/// meeting to join a little deeper, a repeating one a little lighter.
func eventTint(_ e: J) -> Double { e.join.bool ? 0.32 : e.repeats.bool ? 0.15 : 0.24 }
/// Past events fade a little, declined ones more.
func eventFade(_ e: J) -> Double { e.answer.string == "no" ? 0.45 : e.past.bool ? 0.6 : 1 }

/// `.event`: tinted, a bar of the calendar's colour on the left; an invitation
/// not answered yet is an outline, "maybe" is striped, "no" is faded and struck through.
/// Repeating events are lighter, meetings to join deeper with a camera, past ones faded.
struct EventBlock: View {
    let b: J
    let height: CGFloat

    var body: some View {
        let c = b.color.string
        let answer = b.answer.string
        let short = height < 38
        let place = height >= 76 ? b.place.string : ""
        let room = max(1, Int((height - 5 - (place.isEmpty ? 0 : 15)) / 15))
        VStack(alignment: .leading, spacing: 1) {
            if short {
                (Text(b.title.string).font(.system(size: 13, weight: .semibold)) + Text("  \(b.startTime.string)").font(.system(size: 12, weight: .medium)))
                    .strikethrough(answer == "no").lineLimit(1)
            } else {
                Text(b.title.string).font(.system(size: 13, weight: .semibold)).strikethrough(answer == "no").lineLimit(max(1, room - 1))
                HStack(spacing: 4) {
                    Text(b.time.string).font(.system(size: 12)).monospacedDigit()
                    if b.join.bool { Ico("video", size: 12, stroke: 2) }
                    if b.repeats.bool { Ico("repeat", size: 10, stroke: 2) }
                }
                .opacity(0.85).lineLimit(1)
                if !place.isEmpty { Text(place).font(.system(size: 12)).opacity(0.85).lineLimit(1) }
            }
        }
        .foregroundStyle(Palette.mix(c, toward: Palette.text, 0.45))
        .padding(.leading, 11).padding(.trailing, 6).padding(.vertical, short ? 2 : 3)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: short ? .leading : .topLeading)
        .background { BlockFill(color: c, answer: answer, tint: eventTint(b)) }
        .overlay(alignment: .leading) { Rectangle().fill(Palette.color(c).opacity(b.repeats.bool ? 0.55 : 1)).frame(width: 3) }
        .clipShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
        .overlay {
            if answer == "invited" { RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(Palette.color(c).opacity(0.7)) }
        }
        .opacity(eventFade(b))
        .contentShape(Rectangle())
    }
}

/// What is behind an event or an all-day piece, by your answer.
struct BlockFill: View {
    let color: String
    let answer: String
    var tint = 0.24
    var body: some View {
        switch answer {
        case "invited":
            Look.bg
        case "maybe":
            let light = Palette.mix(color, toward: Palette.bg, 0.88), dark = Palette.mix(color, toward: Palette.bg, 0.74)
            Canvas { ctx, size in
                ctx.fill(Path(CGRect(origin: .zero, size: size)), with: .color(light))
                var p = Path()
                var x = -size.height
                while x < size.width + size.height {
                    p.move(to: CGPoint(x: x, y: size.height))
                    p.addLine(to: CGPoint(x: x + size.height, y: 0))
                    x += 17
                }
                ctx.stroke(p, with: .color(dark), lineWidth: 6)
            }
        default:
            Palette.mix(color, toward: Palette.bg, 1 - tint)
        }
    }
}

/// `.pill`: an all-day event; a piece that goes on from the day before or into the next has square ends there.
struct AllDayPill: View {
    @Bindable var model: CalendarModel
    let p: J

    var body: some View {
        let c = p.color.string
        let answer = p.answer.string
        let left: CGFloat = p.continues.bool ? 0 : 6, right: CGFloat = p.goesOn.bool ? 0 : 6
        let shape = UnevenRoundedRectangle(topLeadingRadius: left, bottomLeadingRadius: left, bottomTrailingRadius: right, topTrailingRadius: right, style: .continuous)
        Button { model.open(p.key.string) } label: {
            HStack(spacing: 4) {
                if p.repeats.bool { Ico("repeat", size: 11, stroke: 2.2) }
                Text(p.title.string).strikethrough(answer == "no").lineLimit(1)
            }
            .font(.system(size: 12, weight: .semibold))
            .foregroundStyle(Palette.mix(c, toward: Palette.text, 0.3))
            .padding(.horizontal, 8).padding(.vertical, 3)
            .frame(maxWidth: .infinity, minHeight: 21, alignment: .leading)
            .background {
                if answer == "invited" || answer == "maybe" { BlockFill(color: c, answer: answer) } else { Palette.color(c).opacity(eventTint(p) - 0.02) }
            }
            .clipShape(shape)
            .overlay { if answer == "invited" { shape.stroke(Palette.color(c).opacity(0.7), lineWidth: 1) } }
            .opacity(eventFade(p))
            .padding(.leading, p.continues.bool ? -3 : 0).padding(.trailing, p.goesOn.bool ? -3 : 0)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(p.title.string)
    }
}

/// A task with a date, from Tasks (`.task-mark`): a hollow tick and its title on grey.
struct TaskMark: View {
    let title: String
    var body: some View {
        HStack(spacing: 6) {
            Circle().strokeBorder(Look.text3, lineWidth: 1.5).frame(width: 10, height: 10)
            Text(title).lineLimit(1)
        }
        .font(.system(size: 12)).foregroundStyle(Look.text2)
        .padding(.horizontal, 6).padding(.vertical, 2)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Look.surface2, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
    }
}

// MARK: - Month

struct MonthView: View {
    @Bindable var model: CalendarModel
    let s: J

    var body: some View {
        let cells = s.cells.array
        VStack(spacing: 0) {
            HStack(spacing: 0) {
                ForEach(Array(s.monthHeads.array.enumerated()), id: \.offset) { _, h in
                    Text(h.string.uppercased()).font(.system(size: 13, weight: .semibold)).tracking(0.4).foregroundStyle(Look.text2)
                        .frame(maxWidth: .infinity, alignment: .trailing).padding(.horizontal, 10).padding(.bottom, 8)
                }
            }
            ForEach(0..<max(1, cells.count / 7), id: \.self) { r in
                HStack(spacing: 0) {
                    ForEach(0..<7, id: \.self) { i in
                        if r * 7 + i < cells.count { MonthCell(model: model, c: cells[r * 7 + i], first: i == 0) }
                    }
                }
                .frame(maxHeight: .infinity)
            }
        }
        .padding(.bottom, 20)
    }
}

struct MonthCell: View {
    @Bindable var model: CalendarModel
    let c: J
    let first: Bool
    @State private var hover = false

    var body: some View {
        let today = c.today.bool
        VStack(alignment: .leading, spacing: 2) {
            HStack {
                Spacer()
                Button { model.day(c.date.string) } label: {
                    Text("\(c.day.int)").font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(today ? .white : c.other.bool ? Look.text3 : Look.text)
                        .frame(width: 30, height: 30).background(today ? Palette.color("red") : .clear, in: Circle())
                }
                .buttonStyle(.plain)
            }
            ForEach(c.items.array, id: \.keyId) { it in MonthItem(model: model, it: it) }
            if c.more.int > 0 { Text("+\(c.more.int) more").font(.system(size: 11)).foregroundStyle(Look.text2).padding(.horizontal, 6) }
            ForEach(c.tasks.array, id: \.titleId) { t in TaskMark(title: t.title.string) }
            Spacer(minLength: 0)
        }
        .padding(4)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .background(hover ? Look.hover : .clear)
        .overlay(alignment: .top) { Rectangle().fill(Look.line).frame(height: 1) }
        .overlay(alignment: .leading) { if !first { Rectangle().fill(Look.line).frame(width: 1) } }
        .clipped()
        .contentShape(Rectangle())
        .onTapGesture { model.newEventOn(c.date.string) }
        .onHover { hover = $0 }
    }
}

/// `.mev`: a dot, the time and the title; an all-day one tinted.
struct MonthItem: View {
    @Bindable var model: CalendarModel
    let it: J
    var body: some View {
        let c = it.color.string
        let solid = it.solid.bool
        let answer = it.answer.string
        Button { model.open(it.key.string) } label: {
            HStack(spacing: 6) {
                if !solid {
                    Circle().fill(answer == "invited" ? .clear : Palette.color(c).opacity(it.repeats.bool ? 0.55 : 1)).frame(width: 7, height: 7)
                        .overlay { if answer == "invited" { Circle().strokeBorder(Palette.color(c), lineWidth: 1.5) } }
                }
                (it.time.string.isEmpty ? Text("") : Text("\(it.time.string) ").foregroundStyle(Look.text2).fontWeight(.medium))
                    + Text(it.title.string).strikethrough(answer == "no")
            }
            .font(.system(size: 12, weight: .medium)).lineLimit(1)
            .foregroundStyle(solid ? Palette.mix(c, toward: Palette.text, 0.3) : Look.text)
            .padding(.horizontal, 6).padding(.vertical, 2)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(solid ? Palette.color(c).opacity(eventTint(it) - 0.02) : .clear, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
            .opacity(eventFade(it))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}
