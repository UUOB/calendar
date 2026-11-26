// The list the "Up next" widget draws (mac/Widget): the same upcoming items
// Calendar hands UUOB, kept where the widget's sandbox may read them, and the
// widget told to draw again when they change.

import Foundation
import WidgetKit

enum UpNextFile {
    static func write(_ list: String, hour12: Bool) {
        let dir = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/app.uuob.calendar")
        let items = (try? JSONSerialization.jsonObject(with: Data(list.utf8))) ?? [Any]()
        guard let data = try? JSONSerialization.data(withJSONObject: ["hour12": hour12, "items": items], options: [.sortedKeys]) else { return }
        let file = dir.appendingPathComponent("upnext.json")
        if (try? Data(contentsOf: file)) == data { return }
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        try? data.write(to: file, options: .atomic)
        WidgetCenter.shared.reloadAllTimelines()
    }
}
