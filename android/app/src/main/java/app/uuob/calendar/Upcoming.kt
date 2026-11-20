package app.uuob.calendar

import android.content.BroadcastReceiver
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import app.uuob.kit.UuobAlerts
import org.json.JSONObject
import java.util.Calendar
import java.util.TimeZone

/**
 * What is coming up in Calendar, for the "Up next" widget (UpNext.kt).
 *
 * UUOB keeps the list (the `upcoming` records Calendar writes, see
 * `uuob.upcoming` in the SDK) and hands it over whenever it changes, also when
 * it learns of a change made on another device while Calendar isn't running.
 * It comes only to Calendar, and only Calendar's own items, once the user
 * allowed Calendar in UUOB; anything without UUOB's proof is ignored (`fromUuob`). It
 * is kept on disk, so the widget can draw without UUOB or the web UI running.
 */
object Upcoming {
  /** UUOB to Calendar: here is your list (extra `list`: `{ hour12, items }`). */
  const val LIST = "app.uuob.UPCOMING"
  /** Calendar to UUOB: send me my list (when Calendar has none yet). */
  private const val ASK = "app.uuob.SEND_UPCOMING"

  class Item(
    val title: String, val body: String,
    /** One of the calendar colours ("blue", "red", …). */
    val color: String,
    /** When it starts (midnight of its day for one without a time) and ends (null: it doesn't). */
    val start: Long, val end: Long?, val timed: Boolean,
  )

  private fun prefs(ctx: Context) = ctx.getSharedPreferences("upcoming", Context.MODE_PRIVATE)

  fun save(ctx: Context, list: String) {
    prefs(ctx).edit().putString("list", list).apply()
    UpNext.refresh(ctx)
  }

  /** Asks UUOB for the list, if this phone has never had one (a new install). */
  fun askIfNone(ctx: Context) {
    if (prefs(ctx).contains("list")) return
    try {
      ctx.sendBroadcast(Intent(ASK).setComponent(ComponentName("app.uuob", "app.uuob.AskReceiver")).addFlags(Intent.FLAG_INCLUDE_STOPPED_PACKAGES))
    } catch (e: Exception) {
      // UUOB isn't on this phone: nothing to show until it is.
    }
  }

  private fun list(ctx: Context) = try { JSONObject(prefs(ctx).getString("list", null) ?: "{}") } catch (e: Exception) { JSONObject() }

  fun hour12(ctx: Context) = list(ctx).optBoolean("hour12", false)

  fun items(ctx: Context): List<Item> {
    val list = list(ctx).optJSONArray("items") ?: return emptyList()
    return (0 until list.length()).mapNotNull { i ->
      val o = list.optJSONObject(i) ?: return@mapNotNull null
      val tz = o.optString("tz")
      val date = o.optString("date")
      val time = o.optString("time")
      val timed = time.isNotEmpty()
      val start = instant(date, time.ifEmpty { "00:00" }, tz) ?: return@mapNotNull null
      val endTime = o.optString("end")
      val endDate = o.optString("end_date")
      val end = when {
        endTime.isNotEmpty() && timed -> instant(endDate.ifEmpty { date }, endTime, tz)?.let { if (endDate.isEmpty() && it <= start) it + DAY else it }
        endDate.isNotEmpty() -> instant(endDate, "00:00", tz)?.plus(DAY) // the end of its last day
        else -> null
      }
      Item(o.optString("title"), o.optString("body"), o.optString("color"), start, end, timed)
    }.sortedBy { it.start }
  }

  private const val DAY = 24 * 60 * 60 * 1000L

  /** When the wall clock of `tz` (or this phone's) shows `date` `time`. */
  private fun instant(date: String, time: String, tz: String): Long? {
    val d = date.split("-").mapNotNull { it.toIntOrNull() }
    val t = time.split(":").mapNotNull { it.toIntOrNull() }
    if (d.size != 3 || t.size != 2) return null
    val zone = TimeZone.getTimeZone(tz).takeIf { tz.isNotEmpty() && it.id == tz } ?: TimeZone.getDefault()
    return Calendar.getInstance(zone).apply {
      clear()
      set(d[0], d[1] - 1, d[2], t[0], t[1])
    }.timeInMillis
  }

  /** "9:30" or "9:30 AM" on this phone's clock. */
  fun clock(at: Long, hour12: Boolean): String {
    val c = Calendar.getInstance().apply { timeInMillis = at }
    val h = c.get(Calendar.HOUR_OF_DAY)
    val m = "%02d".format(c.get(Calendar.MINUTE))
    return if (hour12) "${(h + 11) % 12 + 1}:$m ${if (h < 12) "AM" else "PM"}" else "$h:$m"
  }
}

/** UUOB handing over the list (see Upcoming). */
class UpcomingReceiver : BroadcastReceiver() {
  override fun onReceive(ctx: Context, intent: Intent) {
    if (intent.action == Upcoming.LIST && UuobAlerts.fromUuob(intent)) intent.getStringExtra("list")?.let { Upcoming.save(ctx, it) }
  }
}
