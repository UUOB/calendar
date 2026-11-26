// The "Up next" widget on the Mac, as on Android (android/.../UpNext.kt):
// today and tomorrow side by side, the same hours of both from just before
// now, events as blocks as long as they last, a red line at the time now,
// and an event for the whole day as a chip beside its day. Small shows today.
//
// It runs in a sandbox of its own and only reads the list Calendar keeps for
// it (UpNextFile in mac/Sources), the same `upcoming` items Calendar hands
// UUOB. Drawn again at every five minutes on the clock, and whenever
// Calendar changes the list.

import SwiftUI
import WidgetKit

struct UpItem {
    let title: String
    let color: String
    let start: Date
    /// nil: it doesn't end (half an hour is shown).
    let end: Date?
    let timed: Bool
}

enum UpList {
    /// ~/Library/Application Support/app.uuob.calendar/upnext.json, in the real
    /// home (the sandbox's own home is a container).
    static var file: URL {
        let home = getpwuid(getuid()).map { String(cString: $0.pointee.pw_dir) } ?? NSHomeDirectory()
        return URL(fileURLWithPath: home).appendingPathComponent("Library/Application Support/app.uuob.calendar/upnext.json")
    }

    static func load() -> (hour12: Bool, items: [UpItem]) {
        guard let d = try? Data(contentsOf: file), let o = try? JSONSerialization.jsonObject(with: d) as? [String: Any] else { return (false, []) }
        let items = (o["items"] as? [[String: Any]] ?? []).compactMap(item).sorted { $0.start < $1.start }
        return (o["hour12"] as? Bool ?? false, items)
    }

    private static func item(_ o: [String: Any]) -> UpItem? {
        let s = { (k: String) in o[k] as? String ?? "" }
        let (tz, date, time) = (s("tz"), s("date"), s("time"))
        let timed = !time.isEmpty
        guard let start = instant(date, timed ? time : "00:00", tz) else { return nil }
        let (endTime, endDate) = (s("end"), s("end_date"))
        var end: Date?
        if timed && !endTime.isEmpty {
            end = instant(endDate.isEmpty ? date : endDate, endTime, tz).map { endDate.isEmpty && $0 <= start ? $0.addingTimeInterval(86_400) : $0 }
        } else if !endDate.isEmpty {
            // The end of its last day.
            end = instant(endDate, "00:00", tz).flatMap { Calendar.current.date(byAdding: .day, value: 1, to: $0) }
        }
        return UpItem(title: s("title"), color: s("color"), start: start, end: end, timed: timed)
    }

    /// When the wall clock of `tz` (or this Mac's) shows `date` `time`.
    private static func instant(_ date: String, _ time: String, _ tz: String) -> Date? {
        let d = date.split(separator: "-").compactMap { Int($0) }, t = time.split(separator: ":").compactMap { Int($0) }
        guard d.count == 3, t.count >= 2 else { return nil }
        var cal = Calendar(identifier: .gregorian)
        cal.timeZone = TimeZone(identifier: tz) ?? .current
        return cal.date(from: DateComponents(year: d[0], month: d[1], day: d[2], hour: t[0], minute: t[1]))
    }
}

struct UpEntry: TimelineEntry {
    let date: Date
    let hour12: Bool
    let items: [UpItem]
}

struct UpProvider: TimelineProvider {
    func placeholder(in context: Context) -> UpEntry { UpEntry(date: .now, hour12: false, items: []) }
    func getSnapshot(in context: Context, completion: @escaping (UpEntry) -> Void) {
        let (h, items) = UpList.load()
        completion(UpEntry(date: .now, hour12: h, items: items))
    }
    /// Now, then every five minutes on the clock for an hour.
    func getTimeline(in context: Context, completion: @escaping (Timeline<UpEntry>) -> Void) {
        let (h, items) = UpList.load()
        let step: TimeInterval = 300
        let first = (Date.now.timeIntervalSince1970 / step).rounded(.down) * step
        let dates = [Date.now] + (1...12).map { Date(timeIntervalSince1970: first + Double($0) * step) }
        completion(Timeline(entries: dates.map { UpEntry(date: $0, hour12: h, items: items) }, policy: .atEnd))
    }
}

/// UUOB's colours (uuob/ui/tokens.css), light and dark.
private func rgb(_ v: Int) -> Color { Color(red: Double((v >> 16) & 0xff) / 255, green: Double((v >> 8) & 0xff) / 255, blue: Double(v & 0xff) / 255) }
private struct Palette {
    let dark: Bool
    var text: Color { rgb(dark ? 0xf5f5f7 : 0x1d1d1f) }
    var text2: Color { text.opacity(0.62) }
    var line: Color { text.opacity(0.12) }
    var accent: Color { rgb(dark ? 0x0a84ff : 0x007aff) }
    var red: Color { rgb(dark ? 0xff453a : 0xff3b30) }
    var bg: Color { rgb(dark ? 0x0e0e10 : 0xffffff) }
    func color(_ name: String) -> Color {
        switch name {
        case "red": rgb(dark ? 0xff453a : 0xff3b30)
        case "orange": rgb(dark ? 0xff9f0a : 0xff9500)
        case "yellow": rgb(dark ? 0xffd60a : 0xffcc00)
        case "green": rgb(dark ? 0x30d158 : 0x34c759)
        case "teal": rgb(dark ? 0x40c8e0 : 0x30b0c7)
        case "purple": rgb(dark ? 0xbf5af2 : 0xaf52de)
        case "pink": rgb(dark ? 0xff375f : 0xff2d55)
        default: rgb(dark ? 0x0a84ff : 0x007aff)
        }
    }
    var tint: Double { dark ? 0.32 : 0.2 }
}

/// "9:30" or "9:30 AM".
private func clock(_ d: Date, _ hour12: Bool) -> String {
    let c = Calendar.current.dateComponents([.hour, .minute], from: d)
    let (h, m) = (c.hour ?? 0, String(format: "%02d", c.minute ?? 0))
    return hour12 ? "\((h + 11) % 12 + 1):\(m) \(h < 12 ? "AM" : "PM")" : "\(h):\(m)"
}

struct UpNextView: View {
    let e: UpEntry
    @Environment(\.widgetFamily) private var family
    @Environment(\.colorScheme) private var scheme

    private struct Block { let item: UpItem; let a: Date; let b: Date; var column = 0; var columns = 1 }

    var body: some View {
        GeometryReader { g in
            let p = Palette(dark: scheme == .dark)
            let cal = Calendar.current
            let now = e.date
            let days = family == .systemSmall ? 1 : 2
            // Bands: the top one (the days), then an hour each (about 30 points).
            let bands = min(15, max(4, Int(g.size.height / 30)))
            let band = g.size.height / CGFloat(bands)
            let hours = bands - 1
            // The same hours in both days: from just before now, but not past midnight.
            let nowHour = cal.component(.hour, from: now) - (hours >= 4 && cal.component(.minute, from: now) < 30 ? 1 : 0)
            let firstHour = min(max(nowHour, 0), max(0, 24 - hours))
            let shown = min(hours, 24)
            let midnight = { (d: Int) in cal.date(byAdding: .day, value: d, to: cal.startOfDay(for: now))! }
            let at = { (d: Int, h: Int) -> Date in h >= 24 ? midnight(d + 1) : cal.date(bySettingHour: h, minute: 0, second: 0, of: midnight(d))! }
            let pad: CGFloat = 12
            let left = pad + (e.hour12 ? 44 : 34)
            let right = g.size.width - pad
            let gap: CGFloat = 6
            let colW = (right - left - gap * CGFloat(days - 1)) / CGFloat(days)
            let yNow = band + CGFloat(now.timeIntervalSince(at(0, firstHour)) / 3600) * band
            ZStack(alignment: .topLeading) {
                // The hours: a label each (giving way to the time now), and a line across.
                ForEach(0..<shown, id: \.self) { i in
                    let h = firstHour + i
                    let ly = band + CGFloat(i) * band
                    Rectangle().fill(p.line).frame(width: right - left, height: 1).offset(x: left, y: ly)
                    if abs(ly - yNow) > 12 {
                        Text(e.hour12 ? (h == 12 ? "noon" : "\((h + 11) % 12 + 1) \(h < 12 ? "AM" : "PM")") : "\(h):00")
                            .font(.system(size: 10)).foregroundStyle(p.text2).fixedSize()
                            .frame(height: 14).offset(x: pad, y: ly - 7)
                    }
                }
                if days == 2 {
                    Rectangle().fill(p.line).frame(width: 1, height: band * CGFloat(shown + 1) - 6).offset(x: left + colW + gap / 2, y: 6)
                }
                ForEach(0..<days, id: \.self) { day in
                    dayColumn(day: day, x0: left + CGFloat(day) * (colW + gap), colW: colW, band: band,
                              from: at(day, firstHour), to: at(day, firstHour + shown), start: midnight(day), end: midnight(day + 1), p: p)
                }
                // Now: a red line with a dot across today, and the time.
                if yNow >= band && yNow <= g.size.height {
                    Rectangle().fill(p.red).frame(width: colW, height: 1.5).offset(x: left, y: yNow - 0.75)
                    Circle().fill(p.red).frame(width: 7, height: 7).offset(x: left - 3.5, y: yNow - 3.5)
                    Text(clock(now, e.hour12)).font(.system(size: 10, weight: .bold)).foregroundStyle(p.red).fixedSize()
                        .frame(height: 14).offset(x: pad, y: yNow - 7)
                }
            }
            .frame(width: g.size.width, height: g.size.height, alignment: .topLeading)
            .clipped()
        }
        .containerBackground(for: .widget) { Palette(dark: scheme == .dark).bg }
    }

    @ViewBuilder
    private func dayColumn(day: Int, x0: CGFloat, colW: CGFloat, band: CGFloat, from: Date, to: Date, start: Date, end: Date, p: Palette) -> some View {
        let y = { (t: Date) in band + CGFloat(t.timeIntervalSince(from) / 3600) * band }
        let name = start.formatted(.dateTime.weekday(.abbreviated).day()).uppercased()
        let chips = e.items.filter { !$0.timed && $0.start < end && ($0.end ?? .distantFuture) > start }
        // The day, then its first all-day event as a chip (and how many more).
        HStack(spacing: 6) {
            Text(name).font(.system(size: 11, weight: .bold)).foregroundStyle(day == 0 ? p.accent : p.text2).fixedSize()
            if let c = chips.first {
                Text(c.title).font(.system(size: 10)).foregroundStyle(p.text).lineLimit(1)
                    .padding(.horizontal, 6).frame(height: min(18, band - 6))
                    .background(p.color(c.color).opacity(p.tint), in: RoundedRectangle(cornerRadius: 5, style: .continuous))
                if chips.count > 1 { Text("+\(chips.count - 1)").font(.system(size: 10)).foregroundStyle(p.text2).fixedSize() }
            }
        }
        .frame(width: colW - 4, height: band, alignment: .leading).offset(x: x0 + 4, y: 0)
        // Events (one without an end as half an hour), side by side where they overlap.
        ForEach(Array(blocks(from: from, to: to).enumerated()), id: \.offset) { _, bl in
            let w = (colW - 2) / CGFloat(bl.columns)
            let top = y(bl.a) + 1, h = max(4, y(bl.b) - y(bl.a) - 2)
            let col = p.color(bl.item.color)
            VStack(alignment: .leading, spacing: 0) {
                Text(bl.item.title).font(.system(size: 11, weight: .semibold)).foregroundStyle(p.text).lineLimit(1)
                if h >= 30 { Text(clock(bl.item.start, e.hour12)).font(.system(size: 10)).foregroundStyle(p.text2).lineLimit(1) }
            }
            .padding(.leading, 6).padding(.trailing, 2).padding(.top, h >= 20 ? 2 : 0)
            .frame(width: w - 2, height: h, alignment: h >= 20 ? .topLeading : .leading)
            .background(col.opacity(p.tint))
            .overlay(alignment: .leading) { Rectangle().fill(col).frame(width: 3) }
            .clipShape(RoundedRectangle(cornerRadius: 4, style: .continuous))
            .offset(x: x0 + 2 + CGFloat(bl.column) * w, y: top)
        }
    }

    private func blocks(from: Date, to: Date) -> [Block] {
        var bs = e.items.filter(\.timed)
            .map { Block(item: $0, a: max($0.start, from), b: min($0.end ?? $0.start.addingTimeInterval(1800), to)) }
            .filter { $0.b > $0.a }
            .sorted { $0.a < $1.a }
        var i = 0
        while i < bs.count {
            // A run of blocks that overlap one another shares the width.
            var j = i, last = bs[i].b
            while j + 1 < bs.count && bs[j + 1].a < last { j += 1; last = max(last, bs[j].b) }
            var ends: [Date] = []
            for k in i...j {
                if let free = ends.firstIndex(where: { $0 <= bs[k].a }) { bs[k].column = free; ends[free] = bs[k].b } else { bs[k].column = ends.count; ends.append(bs[k].b) }
            }
            for k in i...j { bs[k].columns = ends.count }
            i = j + 1
        }
        return bs
    }
}

struct UpNextWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "upnext", provider: UpProvider()) { e in UpNextView(e: e) }
            .configurationDisplayName("Up next")
            .description("Today and tomorrow, from now on.")
            .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
            .contentMarginsDisabled()
    }
}

@main
struct UpNextBundle: WidgetBundle {
    var body: some Widget { UpNextWidget() }
}
