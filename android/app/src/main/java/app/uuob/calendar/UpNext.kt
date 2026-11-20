package app.uuob.calendar

import android.app.AlarmManager
import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import app.uuob.kit.UuobAlerts
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.RectF
import android.graphics.Typeface
import android.os.Bundle
import android.text.TextPaint
import android.text.TextUtils
import android.util.TypedValue
import android.widget.RemoteViews
import java.text.SimpleDateFormat
import java.util.Calendar
import java.util.Locale

/**
 * The "Up next" widget: today and tomorrow side by side, the same hours of
 * both (from just before now), with what is coming up in Calendar (see
 * Upcoming for where the list comes from): events as blocks as long as they
 * last, a red line at the time now, and events for the whole day as a chip
 * above each day. It is drawn as one picture; a tap opens
 * Calendar. Drawn again when the list changes, the clock or time zone
 * changes, and every quarter of an hour while the phone is awake.
 */
class UpNext : AppWidgetProvider() {
  companion object {
    private const val REDRAW = "app.uuob.calendar.REDRAW"
    private const val QUARTER = 15 * 60 * 1000L

    fun refresh(ctx: Context) {
      val m = AppWidgetManager.getInstance(ctx)
      val ids = m.getAppWidgetIds(ComponentName(ctx, UpNext::class.java))
      for (id in ids) draw(ctx, m, id)
      tick(ctx, ids.isNotEmpty())
    }

    /** Redraws every quarter of an hour (the line for now moves), only while the phone is awake. */
    private fun tick(ctx: Context, on: Boolean) {
      val again = PendingIntent.getBroadcast(ctx, 1, Intent(ctx, UpNext::class.java).setAction(REDRAW), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
      val am = ctx.getSystemService(AlarmManager::class.java)
      if (on) am.setInexactRepeating(AlarmManager.RTC, System.currentTimeMillis() + QUARTER, QUARTER, again) else am.cancel(again)
    }

    private fun draw(ctx: Context, m: AppWidgetManager, id: Int) {
      val o = m.getAppWidgetOptions(id)
      val w = o.getInt(AppWidgetManager.OPTION_APPWIDGET_MIN_WIDTH).takeIf { it > 0 } ?: 320
      val h = o.getInt(AppWidgetManager.OPTION_APPWIDGET_MAX_HEIGHT).takeIf { it > 0 } ?: 180
      // Drawn light and dark: the phone shows the one for its mode, also after it changes.
      val t = Timeline(ctx, w, h, night = false)
      val views = RemoteViews(ctx.packageName, R.layout.widget_up_next)
      views.setImageViewBitmap(R.id.timeline_day, t.bitmap)
      views.setImageViewBitmap(R.id.timeline_night, Timeline(ctx, w, h, night = true).bitmap)
      val open = Intent(ctx, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
      views.setOnClickPendingIntent(R.id.widget, PendingIntent.getActivity(ctx, 0, open, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT))
      m.updateAppWidget(id, views)
    }
  }

  override fun onUpdate(ctx: Context, m: AppWidgetManager, ids: IntArray) {
    Upcoming.askIfNone(ctx)
    for (id in ids) draw(ctx, m, id)
    tick(ctx, true)
  }

  override fun onAppWidgetOptionsChanged(ctx: Context, m: AppWidgetManager, id: Int, options: Bundle) = draw(ctx, m, id)

  override fun onDisabled(ctx: Context) = tick(ctx, false)

  override fun onReceive(ctx: Context, intent: Intent) {
    when (intent.action) {
      REDRAW, Intent.ACTION_TIME_CHANGED, Intent.ACTION_TIMEZONE_CHANGED -> refresh(ctx)
      else -> super.onReceive(ctx, intent)
    }
  }
}

/**
 * The picture, `wDp` × `hDp`: a band on top (each day, and its all-day
 * events) and then the hours from just before now, all of the same height,
 * with a column for today and one for tomorrow.
 */
private class Timeline(val ctx: Context, wDp: Int, hDp: Int, val night: Boolean) {
  val d = ctx.resources.displayMetrics.density
  // UUOB's colours (uuob/ui/tokens.css; the background is res/values*/widget_colors.xml).
  val text = (if (night) 0xFFF5F5F7 else 0xFF1D1D1F).toInt()
  val text2 = alpha(text, 0.62f)
  val accent = (if (night) 0xFF0A84FF else 0xFF007AFF).toInt()
  val red = (if (night) 0xFFFF453A else 0xFFFF3B30).toInt()
  val bitmap: Bitmap = Bitmap.createBitmap((wDp * d).toInt(), (hDp * d).toInt(), Bitmap.Config.ARGB_8888)

  fun dp(v: Float) = v * d
  fun sp(v: Float) = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, v, ctx.resources.displayMetrics)
  fun paint(size: Float, color: Int, bold: Boolean = false) = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
    textSize = sp(size)
    this.color = color
    typeface = Typeface.create(Typeface.DEFAULT, if (bold) Typeface.BOLD else Typeface.NORMAL)
  }
  fun alpha(c: Int, a: Float) = Color.argb((a * 255).toInt(), Color.red(c), Color.green(c), Color.blue(c))
  /** `c` lightly over the widget's background, solid (the hour lines don't show through). */
  val bg = (if (night) 0xFF1C1C1F else 0xFFFFFFFF).toInt()
  fun tinted(c: Int): Int {
    val mix = { a: Int, b: Int -> (a * tint + b * (1 - tint)).toInt() }
    return Color.rgb(mix(Color.red(c), Color.red(bg)), mix(Color.green(c), Color.green(bg)), mix(Color.blue(c), Color.blue(bg)))
  }
  fun fill(color: Int) = Paint(Paint.ANTI_ALIAS_FLAG).apply { this.color = color }

  /** The calendar colours (uuob/ui/tokens.css), light and dark. */
  fun color(name: String): Int = when (name) {
    "red" -> if (night) 0xFFFF453A else 0xFFFF3B30
    "orange" -> if (night) 0xFFFF9F0A else 0xFFFF9500
    "yellow" -> if (night) 0xFFFFD60A else 0xFFFFCC00
    "green" -> if (night) 0xFF30D158 else 0xFF34C759
    "teal" -> if (night) 0xFF40C8E0 else 0xFF30B0C7
    "purple" -> if (night) 0xFFBF5AF2 else 0xFFAF52DE
    "pink" -> if (night) 0xFFFF375F else 0xFFFF2D55
    else -> if (night) 0xFF0A84FF else 0xFF007AFF
  }.toInt()

  val tint = if (night) 0.32f else 0.2f

  init {
    val c = Canvas(bitmap)
    val hour12 = Upcoming.hour12(ctx)
    val now = System.currentTimeMillis()
    val midnight = { daysOn: Int -> Calendar.getInstance().apply {
      set(Calendar.HOUR_OF_DAY, 0); set(Calendar.MINUTE, 0); set(Calendar.SECOND, 0); set(Calendar.MILLISECOND, 0)
      add(Calendar.DATE, daysOn)
    }.timeInMillis }
    val items = Upcoming.items(ctx).filter { it.end == null || it.end > now }

    // Bands: the top one (the days and their all-day events), then an hour each (about 46dp).
    val bands = (hDp / 46).coerceIn(4, 15)
    val band = bitmap.height.toFloat() / bands
    val hours = bands - 1
    // The hours shown, the same in both days: from just before now, but not past midnight
    // (late in the evening it shows the day's last hours).
    val nowCal = Calendar.getInstance().apply { timeInMillis = now }
    val nowHour = nowCal.get(Calendar.HOUR_OF_DAY) - (if (hours >= 4 && nowCal.get(Calendar.MINUTE) < 30) 1 else 0)
    val firstHour = nowHour.coerceIn(0, maxOf(0, 24 - hours))
    val shown = minOf(hours, 24)
    /** When day `day` (0 today, 1 tomorrow) shows hour `h` on the clock (24: its end), also on a daylight saving day. */
    val at = { day: Int, h: Int -> Calendar.getInstance().apply {
      timeInMillis = midnight(day); set(Calendar.HOUR_OF_DAY, h)
    }.timeInMillis }

    val pad = dp(14f)
    val left = pad + dp(if (hour12) 50f else 40f)
    val right = bitmap.width - pad
    val gap = dp(6f)
    val colW = (right - left - gap) / 2

    // The hours: a label each, and a line across both days.
    val line = Paint().apply { color = alpha(text, 0.12f); strokeWidth = dp(1f) }
    val hourPaint = paint(12f, text2)
    val yNow = band + (now - at(0, firstHour)) / 3_600_000f * band
    for (i in 0 until shown) {
      val h = firstHour + i
      val label = when {
        hour12 && h == 12 -> "noon"
        hour12 -> "${(h + 11) % 12 + 1} ${if (h < 12) "AM" else "PM"}"
        else -> "$h:00"
      }
      val ly = band + i * band
      // An hour's label gives way to the time now when they would overlap.
      if (kotlin.math.abs(ly - yNow) > dp(16f)) c.drawText(label, pad, ly + hourPaint.textSize / 3, hourPaint)
      c.drawLine(left, ly, right, ly, line)
    }

    for (day in 0..1) {
      val x0 = left + day * (colW + gap)
      val x1 = x0 + colW
      val dayStart = midnight(day)
      val dayEnd = midnight(day + 1)
      val from = at(day, firstHour)
      val to = at(day, firstHour + shown)
      val y = { t: Long -> band + (t - from) / 3_600_000f * band }

      // The top band: the day, then a chip for its first all-day event (and how many more).
      val dayPaint = paint(13f, if (day == 0) accent else text2, bold = true)
      val name = SimpleDateFormat("EEE d", Locale.getDefault()).format(dayStart).uppercase(Locale.getDefault())
      c.drawText(name, x0 + dp(4f), band / 2 + dayPaint.textSize / 3, dayPaint)
      val chips = items.filter { !it.timed && it.start < dayEnd && (it.end ?: Long.MAX_VALUE) > dayStart }
      val chipPaint = paint(12f, text)
      val chipH = minOf(dp(24f), band - dp(8f))
      val cx = x0 + dp(4f) + dayPaint.measureText(name) + dp(8f)
      if (chips.isNotEmpty()) {
        val more = if (chips.size > 1) "+${chips.size - 1}" else ""
        val moreW = if (more.isEmpty()) 0f else chipPaint.measureText(more) + dp(6f)
        val room = x1 - cx - moreW
        if (room >= dp(44f)) {
          val label = TextUtils.ellipsize(chips[0].title, chipPaint, room - dp(14f), TextUtils.TruncateAt.END).toString()
          val r = RectF(cx, (band - chipH) / 2, cx + chipPaint.measureText(label) + dp(14f), (band + chipH) / 2)
          c.drawRoundRect(r, dp(7f), dp(7f), fill(tinted(color(chips[0].color))))
          c.drawText(label, cx + dp(7f), r.centerY() + chipPaint.textSize / 3, chipPaint)
          if (more.isNotEmpty()) c.drawText(more, r.right + dp(6f), band / 2 + chipPaint.textSize / 3, paint(12f, text2))
        } else c.drawText("+${chips.size}", cx, band / 2 + chipPaint.textSize / 3, paint(12f, text2))
      }

      // Events (one without an end as half an hour), side by side where they overlap.
      class Block(val item: Upcoming.Item, val a: Long, val b: Long) { var column = 0; var columns = 1 }
      val blocks = items.filter { it.timed }
        .map { Block(it, maxOf(it.start, from), minOf(it.end ?: (it.start + 30 * 60_000L), to)) }
        .filter { it.b > it.a }
        .sortedBy { it.a }
      var i = 0
      while (i < blocks.size) {
        // A run of blocks that overlap one another shares the width.
        var j = i
        var end = blocks[i].b
        while (j + 1 < blocks.size && blocks[j + 1].a < end) end = maxOf(end, blocks[++j].b)
        val ends = mutableListOf<Long>()
        for (k in i..j) {
          val free = ends.indexOfFirst { it <= blocks[k].a }
          blocks[k].column = if (free >= 0) free else ends.size
          if (free >= 0) ends[free] = blocks[k].b else ends += blocks[k].b
        }
        for (k in i..j) blocks[k].columns = ends.size
        i = j + 1
      }
      val timePaint = paint(11f, text2)
      val titlePaint = paint(13f, text, bold = true)
      for (bl in blocks) {
        val col = color(bl.item.color)
        val w = (colW - dp(2f)) / bl.columns
        val r = RectF(x0 + dp(2f) + bl.column * w, y(bl.a) + dp(1f), x0 + dp(2f) + (bl.column + 1) * w - dp(2f), y(bl.b) - dp(1f))
        c.drawRoundRect(r, dp(5f), dp(5f), fill(tinted(col)))
        c.drawRect(r.left, r.top, r.left + dp(3f), r.bottom, fill(col))
        c.save()
        c.clipRect(r)
        val tx = r.left + dp(7f)
        val room = r.right - tx - dp(3f)
        val time = Upcoming.clock(bl.item.start, hour12)
        if (r.height() >= titlePaint.textSize + timePaint.textSize + dp(10f)) {
          c.drawText(TextUtils.ellipsize(bl.item.title, titlePaint, room, TextUtils.TruncateAt.END).toString(), tx, r.top + dp(4f) + titlePaint.textSize, titlePaint)
          c.drawText(TextUtils.ellipsize(time, timePaint, room, TextUtils.TruncateAt.END).toString(), tx, r.top + dp(7f) + titlePaint.textSize + timePaint.textSize, timePaint)
        } else {
          c.drawText(TextUtils.ellipsize(bl.item.title, titlePaint, room, TextUtils.TruncateAt.END).toString(), tx, r.centerY() + titlePaint.textSize / 3, titlePaint)
        }
        c.restore()
      }
    }

    // Now: a red line with a dot across today, and the time, if it is in the hours shown.
    if (yNow >= band && yNow <= bitmap.height) {
      val nowPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = red; strokeWidth = dp(1.5f) }
      c.drawLine(left, yNow, left + colW, yNow, nowPaint)
      c.drawCircle(left, yNow, dp(4f), nowPaint)
      val clockPaint = paint(12f, red, bold = true)
      c.drawText(Upcoming.clock(now, hour12).replace(" ", "\u2009"), pad, yNow + clockPaint.textSize / 3, clockPaint)
    }
  }
}
