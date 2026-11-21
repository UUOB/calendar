package app.uuob.calendar

import android.content.Context
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.uuob.calendar.core.ago
import app.uuob.calendar.core.alertOptions
import app.uuob.calendar.core.asksScope
import app.uuob.calendar.core.beforeLabel
import app.uuob.calendar.core.card
import app.uuob.calendar.core.deleteCalendar
import app.uuob.calendar.core.deleteEvent
import app.uuob.calendar.core.dueSubscriptions
import app.uuob.calendar.core.editEvent
import app.uuob.calendar.core.exportCalendar
import app.uuob.calendar.core.importFiles
import app.uuob.calendar.core.moveCursor
import app.uuob.calendar.core.moveOldCalendars
import app.uuob.calendar.core.moveStart
import app.uuob.calendar.core.newEvent
import app.uuob.calendar.core.readFile
import app.uuob.calendar.core.saveCalendar
import app.uuob.calendar.core.saveDefaultAlert
import app.uuob.calendar.core.saveEmails
import app.uuob.calendar.core.saveEvent
import app.uuob.calendar.core.saveSwitch
import app.uuob.calendar.core.switchWithList
import app.uuob.calendar.core.saveViewPrefs
import app.uuob.calendar.core.screen
import app.uuob.calendar.core.setAnswer
import app.uuob.calendar.core.setOwnAlert
import app.uuob.calendar.core.subscribe
import app.uuob.calendar.core.upcomingList
import app.uuob.calendar.core.refresh as refreshLink
import app.uuob.client.friendlyDate
import app.uuob.client.today
import app.uuob.kit.Btn
import app.uuob.kit.BtnKind
import app.uuob.kit.Check
import app.uuob.kit.IconRow
import app.uuob.kit.ProvideText
import app.uuob.kit.WebSwitch
import app.uuob.kit.NumberWheel
import app.uuob.kit.WebSheet
import app.uuob.kit.Colors
import app.uuob.kit.Icon24
import app.uuob.kit.IconBtn
import app.uuob.kit.Pill
import app.uuob.kit.SectionLabel
import app.uuob.kit.Segmented
import app.uuob.kit.SettingsGroup
import app.uuob.kit.SettingsRow
import app.uuob.kit.SettingsSheet
import app.uuob.kit.Stack
import app.uuob.kit.TimeField
import app.uuob.kit.Tokens
import app.uuob.kit.UpdateButton
import app.uuob.kit.Updater
import app.uuob.kit.Uuob
import app.uuob.kit.UuobSettings
import app.uuob.kit.WebField
import app.uuob.kit.WebSheet
import app.uuob.kit.objects
import app.uuob.kit.text
import app.uuob.kit.userMessage
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneOffset

/** Height of an hour in the grid. */
private val HOUR = 52.dp
private val COLORS = listOf("blue", "red", "orange", "yellow", "green", "teal", "purple", "pink")
private val VIEWS = listOf("1d" to "1 day", "2d" to "2 days", "3d" to "3 days", "4d" to "4 days", "5d" to "5 days", "6d" to "6 days", "1w" to "Week", "month" to "Month", "year" to "Year")

private fun JSONObject.arr(k: String): List<JSONObject> = objects(optJSONArray(k)?.toString())
private fun JSONObject.strs(k: String): List<String> = optJSONArray(k)?.let { a -> (0 until a.length()).map { a.optString(it) } } ?: emptyList()

/**
 * Calendar's state: every event, calendar and preference, kept live from
 * UUOB. What the screen shows comes from the core (core/src/app.rs,
 * `screen`); this reads, writes and remembers what the user is looking at.
 */
class CalendarModel(private val ctx: Context) {
  private val client get() = Uuob.client
  var data by mutableStateOf(JSONObject())
  var tasks by mutableStateOf("[]")
  var view by mutableStateOf("")
  var cursor by mutableStateOf(today())
  var narrow by mutableStateOf(true)
  var hidden by mutableStateOf<List<String>?>(null)
  var showTasks by mutableStateOf<Boolean?>(null)
  var views by mutableStateOf<JSONObject?>(null)
  var now by mutableLongStateOf(System.currentTimeMillis())
  var eventsKnown by mutableStateOf(false)
  var prefsKnown by mutableStateOf(false)
  var note by mutableStateOf("")
  // What is open.
  var card by mutableStateOf<JSONObject?>(null)
  var editor by mutableStateOf<JSONObject?>(null)
  var scopeAsk by mutableStateOf<String?>(null)
  var calSheet by mutableStateOf<JSONObject?>(null)
  var calsOpen by mutableStateOf(false)
  var settingsOpen by mutableStateOf(false)
  var subscribing by mutableStateOf(false)
  var importItems by mutableStateOf<List<JSONObject>?>(null)
  var importResult by mutableStateOf<JSONObject?>(null)
  var busy by mutableStateOf("")
  private var refreshedOnce = false
  private var reloading: Job? = null
  private var upcomingJob: Job? = null

  private val main get() = data.arr("containers").firstOrNull { it.optBoolean("main") }?.optString("id") ?: "calendar"
  val events get() = data.optJSONArray("events")?.toString() ?: "[]"

  fun state(): JSONObject {
    val st = JSONObject(data.toString())
    st.put("view", view).put("cursor", cursor).put("narrow", narrow).put("year_ok", true)
      .put("sunday_first", UuobSettings.sundayFirst).put("hour12", UuobSettings.hour12).put("now", now)
      .put("tasks", JSONArray(tasks))
    hidden?.let { st.put("hidden", JSONArray(it)) }
    showTasks?.let { st.put("show_tasks", it) }
    views?.let { st.put("views", it) }
    return st
  }

  val screen by derivedStateOf { JSONObject(screen(state().toString())) }
  private fun range(): Pair<String, String> = screen.strs("days").let { (it.firstOrNull() ?: cursor) to (it.lastOrNull() ?: cursor) }
  val emails get() = JSONArray(screen.strs("emails")).toString()

  suspend fun start() {
    // What was seen last, at once; then UUOB's.
    data = JSONObject()
      .put("events", JSONArray(client.cachedAll("events"))).put("infos", JSONArray(client.cachedAll("calendar")))
      .put("containers", JSONArray(client.cachedContainers() ?: "[]"))
      .put("first", objects(client.cached("calendars", null)).firstOrNull { it.optString("id") == "personal" } ?: JSONObject.NULL)
      .put("prefs", JSONArray(client.cached("prefs", null) ?: "[]")).put("answers", JSONArray(client.cached("answers", null) ?: "[]"))
      .put("alerts", JSONArray(client.cached("alerts", null) ?: "[]")).put("subs", JSONArray(client.cached("subscriptions", null) ?: "[]"))
    Uuob.scope.launch { runCatching { moveOldCalendars(client) } }
    reload()
    Uuob.events.collect { e ->
      if (e.optString("type") == "changed" || (e.optString("type") == "state" && e.optString("state") == "connected")) {
        reloading?.cancel()
        reloading = Uuob.scope.launch { delay(150); reload() }
      }
    }
  }

  suspend fun reload() {
    val d = JSONObject(data.toString())
    runCatching { d.put("containers", JSONArray(client.containers())) }
    val all = JSONObject(client.all("events"))
    d.put("events", all.getJSONArray("records"))
    d.put("infos", JSONObject(client.all("calendar")).getJSONArray("records"))
    runCatching { objects(client.load("calendars", null)).firstOrNull { it.optString("id") == "personal" }?.let { d.put("first", it) } }
    var prefsRead = false
    for ((k, c) in listOf("prefs" to "prefs", "answers" to "answers", "alerts" to "alerts", "subs" to "subscriptions")) {
      runCatching {
        d.put(k, JSONArray(client.load(c, null)))
        if (k == "prefs") prefsRead = true
      }
    }
    data = d
    if (all.optBoolean("complete")) eventsKnown = true
    if (prefsRead) {
      val first = !prefsKnown
      prefsKnown = true
      hidden = null
      views = null
      if (first && JSONObject(app.uuob.calendar.core.prefsOf(d.getJSONArray("prefs").toString())).optBoolean("show_tasks")) {
        runCatching { tasks = client.load("tasks", "tasks") }
      }
    }
    scheduleUpcoming()
    // Calendar links are read again when the app opens, at most every few hours.
    if (!refreshedOnce && prefsRead) {
      refreshedOnce = true
      val due = objects(dueSubscriptions(d.getJSONArray("subs").toString(), d.getJSONArray("containers").toString()))
      for (sub in due) runCatching { refreshLink(client, sub.optString("id"), sub.optString("url"), events, emails) }
    }
  }

  /** What is coming up is written 1.5 s after the last change, once everything has been read. */
  fun scheduleUpcoming() {
    if (!eventsKnown || !prefsKnown) return
    upcomingJob?.cancel()
    upcomingJob = Uuob.scope.launch {
      delay(1500)
      client.upcoming(upcomingList(state().toString()))
    }
  }

  fun launch(work: suspend () -> Unit) {
    Uuob.scope.launch {
      try {
        work()
      } catch (e: Exception) {
        note = e.userMessage
      }
    }
  }

  fun show(v: String) {
    view = v
  }

  fun move(dir: Long) {
    cursor = moveCursor(screen.optString("view"), cursor, dir, UuobSettings.sundayFirst)
  }

  fun day(d: String) {
    cursor = d
    view = screen.optString("dayView")
  }

  fun open(key: String) {
    val (from, to) = range()
    card = JSONObject(card(state().toString(), key, from, to)).takeIf { it.length() > 0 }
  }

  fun edit(key: String) {
    val (from, to) = range()
    card = null
    editor = JSONObject(editEvent(state().toString(), key, from, to))
  }

  fun newEventOn(date: String?, start: String? = null, end: String? = null) {
    val d = date ?: today().let { t ->
      val days = screen.strs("days")
      if (screen.optString("view") == "month" || t in days) t else days.firstOrNull() ?: t
    }
    editor = JSONObject(newEvent(state().toString(), d, start, end))
  }

  fun save(ed: JSONObject, scope: String?) {
    val series = ed.optJSONObject("series") != null
    if (scope == null && series && asksScope(ed.toString())) {
      editor = ed
      scopeAsk = "save"
      return
    }
    editor = null
    scopeAsk = null
    val sc = scope ?: if (series) "all" else null
    launch { saveEvent(client, ed.toString(), sc, events, main) }
  }

  fun delete(ed: JSONObject, scope: String?) {
    if (scope == null && ed.optJSONObject("series") != null) {
      editor = ed
      scopeAsk = "delete"
      return
    }
    editor = null
    scopeAsk = null
    launch { deleteEvent(client, ed.toString(), scope, events, main) }
  }

  private fun replace(list: String, id: String, value: JSONObject?) {
    val items = data.arr(list).filter { it.optString("id") != id }.toMutableList()
    if (value != null) items += value.put("id", id)
    data = JSONObject(data.toString()).put(list, JSONArray(items))
  }

  fun answer(key: String, a: String) {
    replace("answers", key, JSONObject().put("answer", a))
    card = card?.let { JSONObject(it.toString()).put("answer", a) }
    scheduleUpcoming()
    launch { setAnswer(client, key, a) }
  }

  fun ownAlert(key: String, choice: String) {
    replace("alerts", key, if (choice == "default") null else JSONObject().put("before", if (choice == "none") false else choice.toLong()))
    scheduleUpcoming()
    launch { setOwnAlert(client, key, choice) }
  }

  fun toggleCal(id: String) {
    val h = screen.strs("hidden").toMutableList()
    if (id in h) h.remove(id) else h += id
    hidden = h
    val show = screen.optBoolean("showTasks")
    launch { saveViewPrefs(client, JSONArray(h).toString(), show) }
  }

  fun toggleTasks() {
    val on = !screen.optBoolean("showTasks")
    val h = JSONArray(screen.strs("hidden")).toString()
    if (!on) {
      showTasks = false
      tasks = "[]"
      launch { saveViewPrefs(client, h, false) }
      return
    }
    // Showing tasks needs read access to the Tasks app's container.
    launch {
      client.requestAccess("tasks", "read")
      tasks = client.load("tasks", "tasks")
      showTasks = true
      saveViewPrefs(client, h, true)
    }
  }

  fun saveCal(sheet: JSONObject) {
    calSheet = null
    launch { saveCalendar(client, sheet.toString(), data.optJSONArray("infos").toString(), screen.optJSONArray("calendars").toString()) }
  }

  fun deleteCal(id: String, subscribed: Boolean) {
    calSheet = null
    launch { deleteCalendar(client, id, subscribed) }
  }

  fun share(id: String) {
    calSheet = null
    launch { Uuob.share(ctx, id) }
  }

  fun refreshNow(id: String) {
    val sub = data.arr("subs").firstOrNull { it.optString("id") == id } ?: return
    calSheet = null
    note = "Updating…"
    launch {
      val n = refreshLink(client, id, sub.optString("url"), events, emails)
      note = "Updated: $n events."
    }
  }

  fun subscribeTo(url: String, name: String) {
    subscribing = false
    note = "Reading the link…"
    launch {
      subscribe(client, url, name, screen.optJSONArray("calendars")!!.length().toUInt(), emails)
      note = "Subscribed."
    }
  }

  fun readFiles(uris: List<Uri>) {
    calsOpen = false
    importItems = uris.map { u ->
      val name = ctx.contentResolver.query(u, null, null, null, null)?.use { c ->
        val i = c.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
        if (c.moveToFirst() && i >= 0) c.getString(i) else null
      } ?: "Calendar.ics"
      val text = runCatching { ctx.contentResolver.openInputStream(u)?.use { it.readBytes().decodeToString() } }.getOrNull()
      if (text == null) JSONObject().put("name", name).put("error", "This file could not be read as a calendar.")
      else JSONObject(readFile(name, text)).put("target", "new")
    }
  }

  fun runImport(items: List<JSONObject>) {
    importItems = null
    busy = "Importing…"
    launch {
      val r = importFiles(client, JSONArray(items).toString(), screen.optJSONArray("calendars")!!.length().toUInt(), emails)
      busy = ""
      importResult = JSONObject(r)
    }
  }

  fun exportTo(uri: Uri, id: String) {
    val name = screen.arr("calendars").firstOrNull { it.optString("id") == id }?.optString("name") ?: "Calendar"
    val r = JSONObject(exportCalendar(events, id, name))
    note = try {
      ctx.contentResolver.openOutputStream(uri)?.use { it.write(r.optString("text").toByteArray()) }
      "Saved ${r.optInt("count")} events as ${r.optString("name")}."
    } catch (e: Exception) {
      "The file could not be saved."
    }
  }

  fun setSwitch(change: JSONObject) {
    val key = if (narrow) "narrow" else "wide"
    val v = JSONObject(screen.optJSONObject("switch").toString())
    val slot = v.getJSONObject(key)
    change.keys().forEach { slot.put(it, change.get(it)) }
    views = v
    launch { saveSwitch(client, v.toString()) }
  }

  /** The switch's views for this kind of screen, as a list (kept in order by the core). */
  fun setViews(list: List<String>) {
    val key = if (narrow) "narrow" else "wide"
    val v = JSONObject(screen.optJSONObject("switch").toString())
    v.put(key, JSONObject(switchWithList(v.getJSONObject(key).toString(), JSONArray(list).toString())))
    views = v
    launch { saveSwitch(client, v.toString()) }
  }

  fun setBefore(b: Long?) {
    replace("prefs", "alerts", JSONObject().put("before", b ?: JSONObject.NULL))
    scheduleUpcoming()
    launch { saveDefaultAlert(client, b) }
  }

  fun forgetAddress(a: String) {
    val list = screen.strs("emails").filter { it != a }
    replace("prefs", "me", JSONObject().put("emails", JSONArray(list)))
    launch { saveEmails(client, JSONArray(list).toString()) }
  }
}

@Composable
fun CalendarScreen(model: CalendarModel, updater: Updater) {
  LaunchedEffect(Unit) { model.start() }
  // The line for now moves on, and an app left open overnight rolls over.
  LaunchedEffect(Unit) {
    var shown = today()
    while (true) {
      delay(30_000)
      model.now = System.currentTimeMillis()
      val t = today()
      if (t != shown) {
        if (model.cursor == shown) model.cursor = t
        shown = t
      }
    }
  }
  LaunchedEffect(Unit) { while (true) { delay(3_600_000); model.scheduleUpcoming() } }
  val s = model.screen
  var exportId by remember { mutableStateOf<String?>(null) }
  val pickFiles = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { if (it.isNotEmpty()) model.readFiles(it) }
  val saveFile = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("text/calendar")) { uri -> val id = exportId; if (uri != null && id != null) model.exportTo(uri, id) }
  val files = { pickFiles.launch(arrayOf("text/calendar", "text/plain", "application/octet-stream")) }

  BoxWithConstraints(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
    val wide = maxWidth >= 720.dp
    LaunchedEffect(wide) { model.narrow = !wide }
    Row(Modifier.fillMaxSize()) {
      if (wide) {
        Column(Modifier.width(240.dp).fillMaxHeight().background(MaterialTheme.colorScheme.surface).safeDrawingPadding().padding(start = 16.dp, end = 16.dp, top = 28.dp, bottom = 18.dp)) {
          Sidebar(model, s, updater, files, Modifier.weight(1f))
        }
        Box(Modifier.width(1.dp).fillMaxHeight().background(Tokens.line))
      }
      Column(Modifier.weight(1f).fillMaxHeight().safeDrawingPadding().padding(start = if (wide) 28.dp else 12.dp, end = if (wide) 28.dp else 12.dp, top = if (wide) 26.dp else 18.dp)) {
        Header(model, s, wide)
        when (s.optString("view")) {
          "month" -> MonthView(model, s, wide)
          "year" -> YearView(model, s, wide)
          else -> WeekView(model, s)
        }
      }
    }
    if (model.note.isNotEmpty() || model.busy.isNotEmpty()) {
      Text(
        model.busy.ifEmpty { model.note }, textAlign = TextAlign.Center, fontSize = 15.sp,
        modifier = Modifier.align(Alignment.BottomCenter).safeDrawingPadding().padding(20.dp).shadow(8.dp, Pill.shape).clip(Pill.shape)
          .background(MaterialTheme.colorScheme.surface).clickable { model.note = "" }.padding(horizontal = 18.dp, vertical = 10.dp),
      )
      LaunchedEffect(model.note) { if (model.note.isNotEmpty()) { delay(4000); model.note = "" } }
    }
    // Phones: the calendar list rises from the bottom.
    AnimatedVisibility(model.calsOpen && !wide, enter = fadeIn(), exit = fadeOut()) {
      Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.45f)).clickable(remember { MutableInteractionSource() }, null) { model.calsOpen = false })
    }
    AnimatedVisibility(model.calsOpen && !wide, Modifier.align(Alignment.BottomCenter), enter = slideInVertically { it }, exit = slideOutVertically { it }) {
      Column(
        Modifier.fillMaxWidth().heightIn(max = maxHeight * 0.75f).shadow(24.dp, RoundedCornerShape(topStart = 26.dp, topEnd = 26.dp))
          .clip(RoundedCornerShape(topStart = 26.dp, topEnd = 26.dp)).background(MaterialTheme.colorScheme.surface)
          .clickable(remember { MutableInteractionSource() }, null) {}.navigationBarsPadding().padding(horizontal = 16.dp, vertical = 26.dp),
      ) { Sidebar(model, s, updater, files, Modifier) }
    }
  }
  model.card?.let { CardSheet(model, it) }
  model.editor?.let { if (model.scopeAsk == null) EventSheet(model, it) }
  model.scopeAsk?.let { ask -> model.editor?.let { ScopeSheet(model, it, ask) } }
  model.calSheet?.let { CalSheet(model, it) { id -> exportId = id; saveFile.launch(JSONObject(exportCalendar(model.events, id, it.optString("name"))).optString("name")) } }
  if (model.subscribing) SubscribeSheet(model)
  model.importItems?.let { ImportSheet(model, it) }
  model.importResult?.let { r ->
    WebSheet({ model.importResult = null }) {
      Text("Import", fontSize = 22.sp, fontWeight = FontWeight.SemiBold)
      Text(importText(r), color = Tokens.text2, modifier = Modifier.padding(top = 10.dp))
      Btn("Done", { model.importResult = null }, Modifier.fillMaxWidth().padding(top = 18.dp), kind = BtnKind.Primary, lg = true)
    }
  }
  if (model.settingsOpen) CalendarSettings(model, s)
}

private fun importText(r: JSONObject): String {
  val n = r.optInt("events")
  val b = StringBuilder("Imported $n ${if (n == 1) "event" else "events"}.")
  r.text("error")?.let { b.append(" $it Importing the file again picks up where it stopped.") }
  val u = r.optInt("unreadable")
  if (u > 0) b.append("\n\n$u ${if (u == 1) "event repeats" else "events repeat"} in a way this calendar can't show yet; they appear only the first time.")
  val z = r.strs("zones")
  if (z.isNotEmpty()) b.append("\n\nTimes in ${z.joinToString(", ")} were read in the calendar's own time zone.")
  b.append("\n\nImporting the same file again updates these events instead of adding them twice.")
  return b.toString()
}

/** The menu of the sidebar's + (`.menu`): grey, rounded, its icons blue. */
@Composable
private fun MenuItem(icon: String, text: String, onClick: () -> Unit) {
  Row(Modifier.fillMaxWidth().height(40.dp).clip(RoundedCornerShape(9.dp)).clickable(onClick = onClick).padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
    Icon24(icon, size = 17.dp, stroke = 2.2f, tint = MaterialTheme.colorScheme.primary)
    Text(text, fontSize = 15.sp, fontWeight = FontWeight.Medium)
  }
}

@Composable
private fun Sidebar(model: CalendarModel, s: JSONObject, updater: Updater, files: () -> Unit, modifier: Modifier) {
  var menu by remember { mutableStateOf(false) }
  Column(modifier.verticalScroll(rememberScrollState())) {
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(start = 8.dp, bottom = 4.dp)) {
      SectionLabel("Calendars", Modifier.weight(1f))
      UpdateButton(updater)
      Box {
        Box(Modifier.size(30.dp).clip(CircleShape).background(if (menu) MaterialTheme.colorScheme.onSurface.copy(alpha = 0.08f) else Color.Transparent).clickable { menu = !menu }, contentAlignment = Alignment.Center) {
          Icon24("plus", size = 18.dp, stroke = 2.2f, tint = MaterialTheme.colorScheme.primary)
        }
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }, shape = RoundedCornerShape(14.dp), containerColor = Tokens.surface2, modifier = Modifier.width(220.dp).padding(horizontal = 6.dp)) {
          MenuItem("plus", "New calendar") {
            menu = false; model.calsOpen = false
            val n = s.arr("calendars").size
            model.calSheet = JSONObject().put("name", "").put("color", COLORS[(n + 1) % COLORS.size])
          }
          MenuItem("download", "Import a file") { menu = false; files() }
          MenuItem("link", "Subscribe to a link") { menu = false; model.calsOpen = false; model.subscribing = true }
        }
      }
    }
    for (c in s.arr("calendars")) {
      Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).padding(horizontal = 8.dp, vertical = 7.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Check(!c.optBoolean("hidden"), Colors.of(c.optString("color")), { model.toggleCal(c.optString("id")) })
        Text(c.optString("name"), Modifier.weight(1f), fontWeight = FontWeight.Medium, maxLines = 1, overflow = TextOverflow.Ellipsis)
        if (c.optBoolean("subscribed")) Icon24("link", size = 15.dp, tint = Tokens.text3)
        if (c.optBoolean("shared")) Icon24("people", size = 15.dp, tint = Tokens.text3)
        Box(Modifier.size(28.dp).clip(CircleShape).clickable { model.calsOpen = false; model.calSheet = c }, contentAlignment = Alignment.Center) { Icon24("edit", size = 15.dp, tint = Tokens.text2) }
      }
    }
    SectionLabel("From other apps", Modifier.padding(start = 8.dp, top = 28.dp, bottom = 8.dp))
    Row(Modifier.padding(horizontal = 8.dp, vertical = 7.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
      Check(s.optBoolean("showTasks"), Tokens.text2, { model.toggleTasks() })
      Text("Tasks with dates", Modifier.weight(1f), fontWeight = FontWeight.Medium)
      if (!s.optBoolean("showTasks")) Icon24("lock", size = 14.dp, tint = Tokens.text3)
    }
  }
  Row(Modifier.padding(top = 20.dp).clip(Pill.shape).clickable { model.calsOpen = false; model.settingsOpen = true }.padding(horizontal = 10.dp, vertical = 9.dp), verticalAlignment = Alignment.CenterVertically) {
    Icon24("settings", size = 19.dp, tint = Tokens.text2); Spacer(Modifier.width(8.dp)); Text("Settings", fontSize = 15.sp, fontWeight = FontWeight.SemiBold, color = Tokens.text2)
  }
}

@Composable
private fun Header(model: CalendarModel, s: JSONObject, wide: Boolean) {
  val title = s.getJSONObject("title")
  val titleText = @Composable { mod: Modifier ->
    Text(
      buildAnnotatedString {
        if (s.optString("view") == "year") withStyle(SpanStyle(fontWeight = FontWeight.Bold)) { append("${title.optInt("year")}") }
        else {
          withStyle(SpanStyle(fontWeight = FontWeight.Bold)) { append(title.optString("month")) }
          withStyle(SpanStyle(fontWeight = FontWeight.Normal, color = Tokens.text2)) { append(" ${title.optInt("year")}") }
        }
      },
      mod, fontSize = if (wide) 34.sp else 28.sp, letterSpacing = (-0.8).sp, maxLines = 1,
    )
  }
  val vs = s.arr("views")
  val switch = @Composable { mod: Modifier, fill: Boolean ->
    Segmented(vs.map { it.optString("name") }, vs.indexOfFirst { it.optString("id") == s.optString("view") }, mod, fill) { model.show(vs[it].optString("id")) }
  }
  val todayPill = @Composable {
    Box(Modifier.height(32.dp).clip(Pill.shape).background(Tokens.surface2).clickable { model.cursor = today() }.padding(horizontal = 14.dp), contentAlignment = Alignment.Center) {
      Text("Today", fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
    }
  }
  val nav = @Composable {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(2.dp)) {
      IconBtn("left", { model.move(-1) })
      Box(Modifier.height(32.dp).clip(Pill.shape).background(Tokens.surface2).clickable { model.cursor = today() }.padding(horizontal = 14.dp), contentAlignment = Alignment.Center) {
        Text("Today", fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
      }
      IconBtn("right", { model.move(1) })
    }
  }
  val new = @Composable {
    Box(Modifier.size(36.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primary).clickable { model.newEventOn(null) }, contentAlignment = Alignment.Center) {
      Icon24("plus", size = 20.dp, stroke = 2.2f, tint = Color.White)
    }
  }
  if (wide) {
    Row(Modifier.fillMaxWidth().padding(bottom = 18.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
      titleText(Modifier.weight(1f)); switch(Modifier, false); nav(); new()
    }
  } else {
    Column(Modifier.padding(bottom = 14.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
      Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) { titleText(Modifier.weight(1f)); todayPill(); new() }
      // Phones: the views share the row evenly, so all of them fit, with the arrows at its end.
      Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        IconBtn("menu", { model.calsOpen = true })
        switch(Modifier.weight(1f), true)
        IconBtn("left", { model.move(-1) })
        IconBtn("right", { model.move(1) })
      }
    }
  }
}

/** `color-mix(in srgb, a p%, b)`. */
private fun mix(a: Color, p: Float, b: Color) = Color(a.red * p + b.red * (1 - p), a.green * p + b.green * (1 - p), a.blue * p + b.blue * (1 - p), 1f)

/** The look of an event by your answer: invited is an outline, maybe striped, no faded and struck through. */
/** How strongly an event is tinted, by kind: a meeting to join a little deeper, a repeating one lighter. */
private fun tintOf(e: JSONObject, base: Float = 0.24f): Float = when {
  e.optBoolean("join") -> base + 0.08f
  e.optBoolean("repeats") -> base - 0.09f
  else -> base
}
/** Past events fade a little, declined ones more. */
private fun fadeOf(e: JSONObject): Float = if (e.text("answer") == "no") 0.45f else if (e.optBoolean("past")) 0.6f else 1f

@Composable
private fun Modifier.eventLook(c: Color, answer: String?, strength: Float, shape: RoundedCornerShape): Modifier {
  val bg = MaterialTheme.colorScheme.background
  return when (answer) {
    "invited" -> this.background(bg, shape).border(1.dp, c.copy(alpha = 0.7f), shape)
    "maybe" -> this.drawBehind {
      val a = mix(c, 0.26f, bg); val b = mix(c, 0.12f, bg)
      drawRect(b)
      val step = 12.dp.toPx(); var x = -size.height
      while (x < size.width) { drawLine(a, Offset(x, size.height), Offset(x + size.height, 0f), strokeWidth = 6.dp.toPx() / 1.414f); x += step / 1.414f * 1.414f }
    }
    else -> this.background(mix(c, strength, bg), shape)
  }
}

@Composable
private fun WeekView(model: CalendarModel, s: JSONObject) {
  val cols = s.arr("columns")
  val scroll = rememberScrollState()
  val density = LocalDensity.current
  LaunchedEffect(s.optString("view"), s.optString("cursor")) { scroll.scrollTo(with(density) { (HOUR * 7.5f).roundToPx() }) }
  val gutter = 56.dp
  val red = Colors.of("red")
  Row(Modifier.fillMaxWidth()) {
    Spacer(Modifier.width(gutter))
    cols.forEach { c ->
      val today = c.optBoolean("today")
      Row(
        Modifier.weight(1f).clickable(enabled = cols.size > 1 && s.optString("dayView") != s.optString("view")) { model.day(c.optString("date")) }.padding(top = 6.dp, bottom = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterHorizontally), verticalAlignment = Alignment.CenterVertically,
      ) {
        if (cols.size <= 4) Text(c.optString("weekday").uppercase(), fontSize = 13.sp, fontWeight = FontWeight.Medium, letterSpacing = 0.4.sp, color = if (today) red else Tokens.text2)
        Box(Modifier.size(34.dp).clip(CircleShape).background(if (today) red else Color.Transparent), contentAlignment = Alignment.Center) {
          Text("${c.optInt("day")}", fontSize = 20.sp, fontWeight = FontWeight.SemiBold, color = if (today) Color.White else MaterialTheme.colorScheme.onSurface)
        }
      }
    }
  }
  // The all-day row.
  Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min)) {
    Text("all-day", fontSize = 11.sp, color = Tokens.text3, textAlign = TextAlign.End, modifier = Modifier.width(gutter).padding(top = 6.dp, end = 8.dp))
    cols.forEach { c ->
      Column(Modifier.weight(1f).fillMaxHeight().drawBehind { drawLine(Color.Gray.copy(alpha = 0.15f), Offset(0f, 0f), Offset(0f, size.height), 1f) }.padding(horizontal = 2.dp, vertical = 3.dp).heightIn(min = 24.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
        c.arr("allDay").forEach { p ->
          val col = Colors.of(p.optString("color"))
          val shape = RoundedCornerShape(topStart = if (p.optBoolean("continues")) 0.dp else 6.dp, bottomStart = if (p.optBoolean("continues")) 0.dp else 6.dp, topEnd = if (p.optBoolean("goesOn")) 0.dp else 6.dp, bottomEnd = if (p.optBoolean("goesOn")) 0.dp else 6.dp)
          val answer = p.text("answer")
          Row(
            Modifier.fillMaxWidth().heightIn(min = 21.dp).alpha(fadeOf(p)).clip(shape).eventLook(col, answer, tintOf(p, 0.22f), shape).clickable { model.open(p.optString("key")) }.padding(horizontal = 8.dp, vertical = 3.dp),
            verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp),
          ) {
            val fg = mix(col, 0.7f, MaterialTheme.colorScheme.onSurface)
            if (p.optBoolean("repeats")) Icon24("repeat", size = 11.dp, stroke = 2.4f, tint = fg)
            Text(p.optString("title"), fontSize = 12.sp, fontWeight = FontWeight.SemiBold, color = fg, maxLines = 1, overflow = TextOverflow.Ellipsis, textDecoration = if (answer == "no") TextDecoration.LineThrough else null)
          }
        }
        c.arr("tasksAllDay").forEach { t -> TaskMark(t.optString("title"), Modifier.fillMaxWidth()) }
      }
    }
  }
  Box(Modifier.fillMaxWidth().height(1.dp).background(Tokens.line.copy(alpha = 0.14f / 0.08f * Tokens.line.alpha)))
  Row(Modifier.fillMaxSize().verticalScroll(scroll)) {
    Column(Modifier.width(gutter)) {
      for (h in 0 until 24) Box(Modifier.height(HOUR).fillMaxWidth()) {
        if (h > 0) Text(app.uuob.client.friendlyTime("%02d:00".format(h), UuobSettings.hour12), fontSize = 11.sp, color = Tokens.text3, maxLines = 1, modifier = Modifier.align(Alignment.TopEnd).offset(y = (-7).dp).padding(end = 8.dp))
      }
    }
    cols.forEach { c -> DayColumn(model, c, s.optInt("nowMinute"), Modifier.weight(1f)) }
  }
}

/** `.task-mark` / `.pill.task`: a task with a date, a small ring and its title. */
@Composable
private fun TaskMark(title: String, modifier: Modifier) {
  Row(modifier.clip(RoundedCornerShape(6.dp)).background(Tokens.surface2).padding(horizontal = 6.dp, vertical = 2.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
    Box(Modifier.size(10.dp).border(1.5.dp, Tokens.text3, CircleShape))
    Text(title, fontSize = 12.sp, color = Tokens.text2, maxLines = 1, overflow = TextOverflow.Ellipsis)
  }
}

@Composable
private fun DayColumn(model: CalendarModel, c: JSONObject, nowMinute: Int, modifier: Modifier) {
  val density = LocalDensity.current
  var drag by remember { mutableStateOf<Pair<Int, Int>?>(null) }
  val hourPx = with(density) { HOUR.toPx() }
  fun snap(y: Float) = ((y / hourPx * 4).toInt() * 15).coerceIn(0, 1440)
  val red = Colors.of("red")
  val line = Tokens.line
  val accent = MaterialTheme.colorScheme.primary
  BoxWithConstraints(
    modifier.height(HOUR * 24)
      .background(if (c.optBoolean("today")) accent.copy(alpha = 0.03f) else Color.Transparent)
      // A finger rests on an empty part, then drags out a new event.
      .pointerInput(c.optString("date")) {
        detectDragGesturesAfterLongPress(
          onDragStart = { o -> val m = snap(o.y).coerceAtMost(1425); drag = m to m + 60 },
          onDrag = { change, _ -> drag = drag?.let { (from, _) -> from to snap(change.position.y).coerceAtLeast(from + 15).coerceAtMost(1440) } },
          onDragEnd = {
            drag?.let { (from, to) -> model.newEventOn(c.optString("date"), "%02d:%02d".format(from / 60, from % 60), "%02d:%02d".format(minOf(to, 1439) / 60, minOf(to, 1439) % 60)) }
            drag = null
          },
          onDragCancel = { drag = null },
        )
      },
  ) {
    val w = maxWidth
    Canvas(Modifier.fillMaxSize()) {
      for (h in 0 until 24) drawLine(line, Offset(0f, h * hourPx), Offset(size.width, h * hourPx), 1f)
      drawLine(line, Offset(0f, 0f), Offset(0f, size.height), 1f)
    }
    c.arr("timed").forEach { b ->
      val lanes = maxOf(1, b.optInt("lanes"))
      val lane = b.optInt("lane")
      val top = HOUR * (b.optInt("start") / 60f)
      val h = maxOf(22.dp, HOUR * ((b.optInt("end") - b.optInt("start")) / 60f) - 2.dp)
      val bw = w / lanes
      val col = Colors.of(b.optString("color"))
      val answer = b.text("answer")
      val shape = RoundedCornerShape(7.dp)
      val src = remember { MutableInteractionSource() }
      val pressed by src.collectIsPressedAsState()
      Box(
        Modifier.offset(x = bw * lane + 2.dp, y = top).width(bw - 4.dp).height(h).scale(if (pressed) 0.98f else 1f)
          .alpha(fadeOf(b)).clip(shape).eventLook(col, answer, tintOf(b), shape)
          .clickable(src, null) { model.open(b.optString("key")) },
      ) {
        Box(Modifier.width(3.dp).fillMaxHeight().background(col.copy(alpha = if (b.optBoolean("repeats")) 0.55f else 1f)))
        val fg = mix(col, 0.55f, MaterialTheme.colorScheme.onSurface)
        val short = h < 38.dp
        Column(Modifier.padding(start = 8.dp, end = 4.dp, top = if (short) 2.dp else 3.dp).fillMaxHeight(), verticalArrangement = if (short) Arrangement.Center else Arrangement.spacedBy(1.dp)) {
          val where = if (h >= 76.dp) b.optString("place") else ""
          val room = maxOf(1, ((h.value - 5 - if (where.isNotEmpty()) 15 else 0) / 15).toInt())
          val timeLine = h >= 38.dp && room > 1
          Text(
            buildAnnotatedString {
              append(b.optString("title"))
              if (short) withStyle(SpanStyle(fontSize = 12.sp, fontWeight = FontWeight.Medium)) { append(" " + b.optString("startTime")) }
            },
            fontSize = 13.sp, fontWeight = FontWeight.SemiBold, lineHeight = 15.sp, color = fg, maxLines = if (timeLine) room - 1 else room, overflow = TextOverflow.Ellipsis,
            textDecoration = if (answer == "no") TextDecoration.LineThrough else null,
          )
          if (timeLine) Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(b.optString("time"), fontSize = 12.sp, lineHeight = 15.sp, color = fg.copy(alpha = 0.85f), maxLines = 1)
            // A camera for a meeting to join, arrows for a repeating event.
            if (b.optBoolean("join")) Icon24("video", size = 12.dp, stroke = 2f, tint = fg.copy(alpha = 0.85f))
            if (b.optBoolean("repeats")) Icon24("repeat", size = 10.dp, stroke = 2.2f, tint = fg.copy(alpha = 0.85f))
          }
          if (where.isNotEmpty()) Text(where, fontSize = 12.sp, lineHeight = 15.sp, color = fg.copy(alpha = 0.85f), maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
      }
    }
    c.arr("tasksTimed").forEach { t -> TaskMark(t.optString("title"), Modifier.offset(y = HOUR * (t.optInt("minute") / 60f)).padding(horizontal = 2.dp).fillMaxWidth()) }
    drag?.let { (from, to) ->
      val shape = RoundedCornerShape(7.dp)
      Box(Modifier.offset(y = HOUR * (from / 60f)).fillMaxWidth().height(HOUR * ((to - from) / 60f)).padding(horizontal = 2.dp).alpha(0.85f).clip(shape).background(mix(accent, 0.24f, MaterialTheme.colorScheme.background)).border(1.dp, accent, shape)) {
        val hm = { m: Int -> app.uuob.client.friendlyTime("%02d:%02d".format(minOf(m, 1439) / 60, minOf(m, 1439) % 60), UuobSettings.hour12) }
        Text("${hm(from)} – ${hm(to)}", fontSize = 12.sp, color = mix(accent, 0.55f, MaterialTheme.colorScheme.onSurface), modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp))
      }
    }
    if (c.optBoolean("today")) {
      val y = HOUR * (nowMinute / 60f)
      Box(Modifier.offset(y = y - 1.dp).fillMaxWidth().height(2.dp).background(red))
      Box(Modifier.offset(x = (-5).dp, y = y - 5.dp).size(10.dp).clip(CircleShape).background(red))
    }
  }
}

private val MONTH_NAMES = listOf("January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December")

/** How strongly a day in the year is shaded (0 none … 4 five hours or more planned). */
@Composable
private fun planColor(level: Int, other: Boolean): Color = when {
  other -> Color.Transparent
  level == 0 -> Tokens.surface2
  else -> MaterialTheme.colorScheme.primary.copy(alpha = listOf(0.28f, 0.5f, 0.75f, 1f)[level - 1])
}

/** One day of the year: a rounded square, ringed when it is today; tapping opens it. */
@Composable
private fun PlanCell(model: CalendarModel, c: JSONObject, modifier: Modifier) {
  val other = c.optBoolean("other")
  Box(
    modifier.aspectRatio(1f).clip(RoundedCornerShape(3.dp)).background(planColor(c.optInt("level"), other))
      .then(if (c.optBoolean("today")) Modifier.border(1.5.dp, Colors.of("red"), RoundedCornerShape(3.dp)) else Modifier)
      .then(if (other) Modifier else Modifier.clickable { model.day(c.optString("date")) }),
  )
}

/** The year as a heat map of planned time (like GitHub's year of activity): phones a week a row, wider screens a week a column. */
@Composable
private fun YearView(model: CalendarModel, s: JSONObject, wide: Boolean) {
  val y = s.optJSONObject("year") ?: return
  val wa = y.optJSONArray("weeks") ?: JSONArray()
  val weeks = (0 until wa.length()).map { i -> wa.getJSONArray(i).let { a -> (0 until a.length()).map { a.getJSONObject(it) } } }
  val months = y.arr("months").associate { it.optInt("week") to it.optString("name") }
  val heads = s.strs("monthHeads")
  val gap = if (wide) 3.dp else 4.dp
  Column(Modifier.fillMaxSize().padding(bottom = 16.dp)) {
    if (wide) {
      Row(Modifier.fillMaxWidth().padding(start = 34.dp, bottom = 6.dp)) {
        weeks.indices.forEach { w -> Box(Modifier.weight(1f)) { months[w]?.let { Text(it, fontSize = 12.sp, color = Tokens.text2, maxLines = 1, softWrap = false) } } }
      }
      Row(Modifier.fillMaxWidth()) {
        Column(Modifier.width(34.dp), verticalArrangement = Arrangement.spacedBy(gap)) {
          heads.forEachIndexed { i, h -> Box(Modifier.weight(1f, false)) { if (i % 2 == 1) Text(h.take(3), fontSize = 11.sp, color = Tokens.text3) } }
        }
        weeks.forEach { week ->
          Column(Modifier.weight(1f).padding(end = gap), verticalArrangement = Arrangement.spacedBy(gap)) {
            week.forEach { PlanCell(model, it, Modifier.fillMaxWidth()) }
          }
        }
      }
    } else {
      // Phones: the twelve months as small grids, three to a row, so the whole year fits.
      val flat = weeks.flatten()
      val byMonth = flat.withIndex().filter { !it.value.optBoolean("other") }.groupBy { it.value.optString("date").substring(5, 7) }
      Column(Modifier.weight(1f).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(14.dp)) {
        byMonth.values.chunked(3).forEach { row ->
          Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            row.forEach { days ->
              Column(Modifier.weight(1f)) {
                val m = days.first().value.optString("date").substring(5, 7).toInt()
                Text(MONTH_NAMES[m - 1], fontSize = 13.sp, fontWeight = FontWeight.SemiBold, color = Tokens.text2, modifier = Modifier.padding(bottom = 5.dp))
                // Blanks before the 1st, so each weekday keeps its column.
                val cells: List<JSONObject?> = List(days.first().index % 7) { null } + days.map { it.value }
                cells.chunked(7).forEach { week ->
                  Row(Modifier.fillMaxWidth().padding(bottom = 2.dp), horizontalArrangement = Arrangement.spacedBy(2.dp)) {
                    (0 until 7).forEach { i ->
                      val c = week.getOrNull(i)
                      if (c == null) Spacer(Modifier.weight(1f).aspectRatio(1f)) else PlanCell(model, c, Modifier.weight(1f))
                    }
                  }
                }
              }
            }
            repeat(3 - row.size) { Spacer(Modifier.weight(1f)) }
          }
        }
      }
    }
    Row(Modifier.fillMaxWidth().padding(top = 14.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
      Text("${y.optInt("hours")} h planned", fontSize = 13.sp, color = Tokens.text2, modifier = Modifier.weight(1f))
      Text("Less", fontSize = 12.sp, color = Tokens.text3)
      (0..4).forEach { Box(Modifier.size(12.dp).clip(RoundedCornerShape(3.dp)).background(planColor(it, false))) }
      Text("More", fontSize = 12.sp, color = Tokens.text3)
    }
  }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun MonthView(model: CalendarModel, s: JSONObject, wide: Boolean) {
  val cells = s.arr("cells")
  val red = Colors.of("red")
  Row(Modifier.fillMaxWidth().padding(bottom = if (wide) 8.dp else 6.dp)) {
    s.strs("monthHeads").forEach { Text(it.uppercase().let { h -> if (wide) h else h.take(1) }, Modifier.weight(1f).padding(horizontal = if (wide) 10.dp else 0.dp), textAlign = if (wide) TextAlign.End else TextAlign.Center, fontSize = 13.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.4.sp, color = Tokens.text2) }
  }
  Column(Modifier.fillMaxSize().padding(bottom = 20.dp)) {
    cells.chunked(7).forEach { week ->
      Row(Modifier.weight(1f).fillMaxWidth()) {
        week.forEachIndexed { i, c ->
          val line = Tokens.line
          Column(
            Modifier.weight(1f).fillMaxHeight()
              .drawBehind { drawLine(line, Offset(0f, 0f), Offset(size.width, 0f), 1f); if (i > 0) drawLine(line, Offset(0f, 0f), Offset(0f, size.height), 1f) }
              .clickable { model.newEventOn(c.optString("date")) }.padding(4.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp),
          ) {
            val today = c.optBoolean("today")
            Box(Modifier.align(if (wide) Alignment.End else Alignment.CenterHorizontally).size(30.dp).clip(CircleShape).background(if (today) red else Color.Transparent).clickable { model.day(c.optString("date")) }, contentAlignment = Alignment.Center) {
              Text("${c.optInt("day")}", fontSize = 15.sp, fontWeight = FontWeight.SemiBold, color = if (today) Color.White else if (c.optBoolean("other")) Tokens.text3 else MaterialTheme.colorScheme.onSurface)
            }
            val items = c.arr("items")
            if (wide) {
              items.forEach { it2 ->
                val col = Colors.of(it2.optString("color"))
                val solid = it2.optBoolean("solid")
                val answer = it2.text("answer")
                Row(
                  Modifier.fillMaxWidth().alpha(fadeOf(it2)).clip(RoundedCornerShape(6.dp)).background(if (solid) col.copy(alpha = tintOf(it2, 0.22f)) else Color.Transparent).clickable { model.open(it2.optString("key")) }.padding(horizontal = 6.dp, vertical = 2.dp),
                  verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                  if (!solid) Box(Modifier.size(9.dp).clip(CircleShape).then(if (answer == "invited") Modifier.border(1.5.dp, col, CircleShape) else Modifier.background(col.copy(alpha = if (it2.optBoolean("repeats")) 0.55f else 1f))))
                  Text(
                    buildAnnotatedString {
                      if (it2.optString("time").isNotEmpty()) withStyle(SpanStyle(color = Tokens.text2, fontWeight = FontWeight.Medium)) { append(it2.optString("time") + " ") }
                      append(it2.optString("title"))
                    },
                    fontSize = 12.sp, fontWeight = FontWeight.Medium, maxLines = 1, overflow = TextOverflow.Ellipsis,
                    color = if (solid) mix(col, 0.7f, MaterialTheme.colorScheme.onSurface) else MaterialTheme.colorScheme.onSurface,
                    textDecoration = if (answer == "no") TextDecoration.LineThrough else null,
                  )
                }
              }
            } else {
              // Phones: the same lines without their text (`.mev .mt { display: none }`): a dot
              // per event under the other, and a pale full-width bar for all-day ones.
              items.forEach { it2 ->
                val col = Colors.of(it2.optString("color"))
                val solid = it2.optBoolean("solid")
                val answer = it2.text("answer")
                Box(
                  // An all-day line without its text is just its padding: a thin bar.
                  Modifier.fillMaxWidth().height(if (solid) 4.dp else 13.dp).alpha(if (answer == "no") 0.45f else 1f).clip(RoundedCornerShape(6.dp)).background(if (solid) col.copy(alpha = 0.22f) else Color.Transparent).clickable { model.open(it2.optString("key")) }.padding(horizontal = 6.dp),
                  contentAlignment = Alignment.CenterStart,
                ) {
                  if (!solid) Box(Modifier.size(9.dp).clip(CircleShape).then(if (answer == "invited") Modifier.border(1.5.dp, col, CircleShape) else Modifier.background(col)))
                }
              }
            }
            if (c.optInt("more") > 0) Text("+${c.optInt("more")} more", fontSize = 11.sp, lineHeight = 13.sp, color = Tokens.text2, modifier = Modifier.padding(horizontal = 6.dp))
            if (wide) c.arr("tasks").forEach { t -> TaskMark(t.optString("title"), Modifier.fillMaxWidth()) }
          }
        }
      }
    }
  }
}

/** A `<select>`: the choice in a field, a menu to change it. */
@Composable
private fun Select(options: List<Pair<String, String>>, current: String, modifier: Modifier = Modifier, plain: Boolean = false, small: Boolean = false, onPick: (String) -> Unit) {
  var menu by remember { mutableStateOf(false) }
  Box(modifier) {
    Row(
      // Plain (in a row): only as wide as its text, so the row's name keeps its room.
      (if (plain) Modifier else Modifier.fillMaxWidth().height(46.dp).clip(Tokens.field).background(Tokens.surface2).border(1.dp, Tokens.line, Tokens.field).padding(horizontal = 14.dp))
        .clickable { menu = true },
      verticalAlignment = Alignment.CenterVertically,
    ) {
      Text(options.firstOrNull { it.first == current }?.second ?: current, Modifier.weight(1f, fill = !plain), fontSize = if (small) 13.sp else 15.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
      Spacer(Modifier.width(6.dp))
      Icon24("chevron", Modifier.rotate(90f), size = 16.dp, tint = Tokens.text2)
    }
    DropdownMenu(expanded = menu, onDismissRequest = { menu = false }, shape = RoundedCornerShape(14.dp), containerColor = Tokens.surface2) {
      options.forEach { (v, l) -> DropdownMenuItem(text = { Text(l, fontSize = 15.sp, fontWeight = if (v == current) FontWeight.SemiBold else FontWeight.Normal) }, onClick = { menu = false; onPick(v) }) }
    }
  }
}

/** The alert picker: the usual times, or Custom… (days, hours and minutes before, up to a week). */
@Composable
private fun AlertPick(options: List<JSONObject>, current: String, plain: Boolean = false, onPick: (String) -> Unit) {
  var custom by remember { mutableStateOf(false) }
  // A time chosen with Custom… shows by its name, though the list it came with doesn't have it.
  val known = options.map { it.optString("value") to it.optString("label") }
  val listed = if (known.any { it.first == current } || current.toLongOrNull() == null) known else known + (current to beforeLabel(current.toLong()))
  Select(listed + ("custom" to "Custom…"), current, plain = plain) { v -> if (v == "custom") custom = true else onPick(v) }
  if (custom) {
    val m = current.toLongOrNull()?.coerceAtLeast(0) ?: 15L
    var d by remember { mutableStateOf((m / 1440).toInt()) }
    var h by remember { mutableStateOf(((m % 1440) / 60).toInt()) }
    var mi by remember { mutableStateOf(((m % 60) / 5 * 5).toInt()) }
    WebSheet({ custom = false }, padding = 22.dp) {
      Text("Alert", fontSize = 22.sp, fontWeight = FontWeight.SemiBold, letterSpacing = (-0.4).sp)
      Row(Modifier.fillMaxWidth().padding(top = 18.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        NumberWheel((0..7).toList(), d, "days", { d = it }, Modifier.weight(1f))
        NumberWheel((0..23).toList(), h, "hours", { h = it }, Modifier.weight(1f))
        NumberWheel((0..55 step 5).toList(), mi, "minutes", { mi = it }, Modifier.weight(1f))
      }
      Text("before the event", color = Tokens.text2, textAlign = TextAlign.Center, modifier = Modifier.fillMaxWidth().padding(top = 14.dp))
      val total = minOf(d * 1440L + h * 60L + mi, 7 * 1440L)
      Btn("Done", { if (total > 0) onPick(total.toString()); custom = false }, Modifier.fillMaxWidth().padding(top = 18.dp), kind = BtnKind.Primary, lg = true, enabled = total > 0)
    }
  }
}

/** `.row` on the card: an icon, a line of text, a line below. */
@Composable
private fun CardRow(icon: String, modifier: Modifier = Modifier, color: Color? = null, last: Boolean = false, content: @Composable RowScope.() -> Unit) {
  Column(modifier) {
    Row(Modifier.fillMaxWidth().heightIn(min = 46.dp).padding(horizontal = 2.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
      Icon24(icon, size = 20.dp, stroke = 1.8f, tint = color ?: Tokens.text2)
      content()
    }
    if (!last) Box(Modifier.fillMaxWidth().height(1.dp).background(Tokens.line))
  }
}

@Composable
private fun CardSheet(model: CalendarModel, c: JSONObject) {
  val uri = LocalUriHandler.current
  var peopleOpen by remember(c.optString("answerKey")) { mutableStateOf(false) }
  var joiningOpen by remember(c.optString("answerKey")) { mutableStateOf(false) }
  WebSheet({ model.card = null }, padding = 22.dp) {
    Row(verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
      Text(c.optString("title"), fontSize = 22.sp, fontWeight = FontWeight.SemiBold, letterSpacing = (-0.4).sp, modifier = Modifier.weight(1f).padding(top = 4.dp, end = 6.dp))
      if (c.optBoolean("canEdit")) IconBtn("edit", { model.edit(c.getJSONObject("event").optString("key")) }, size = 19.dp)
      IconBtn("close", { model.card = null }, size = 19.dp)
    }
    Text(c.optString("when"), fontSize = 16.sp, fontWeight = FontWeight.Medium, modifier = Modifier.padding(top = 10.dp, bottom = 2.dp))
    c.text("repeat")?.let { Text(it, fontSize = 13.sp, color = Tokens.text2, modifier = Modifier.padding(top = 2.dp)) }
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 2.dp)) {
      Box(Modifier.size(9.dp).clip(CircleShape).background(Colors.of(c.optString("color")))); Spacer(Modifier.width(7.dp))
      Text(c.optString("calendar"), fontSize = 13.sp, color = Tokens.text2)
    }
    c.optJSONObject("meeting")?.let { m ->
      Row(
        Modifier.fillMaxWidth().padding(top = 18.dp).height(50.dp).clip(RoundedCornerShape(14.dp)).background(MaterialTheme.colorScheme.primary).clickable { runCatching { uri.openUri(m.optString("url")) } }.padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp),
      ) {
        Icon24("video", size = 20.dp, stroke = 1.8f, tint = Color.White)
        Text("Join ${m.optString("name")}", color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.weight(1f))
        Text(m.optString("host"), color = Color.White.copy(alpha = 0.8f), fontSize = 13.sp, fontWeight = FontWeight.Medium)
      }
    }
    Spacer(Modifier.height(6.dp))
    if (c.optString("place").isNotEmpty()) CardRow("place") { Text(c.optString("place"), fontSize = 15.sp, modifier = Modifier.weight(1f)) }
    c.text("url")?.let { u -> CardRow("link", Modifier.clickable { runCatching { uri.openUri(u) } }, color = MaterialTheme.colorScheme.primary) { Text(c.optString("url_shown"), fontSize = 15.sp, color = MaterialTheme.colorScheme.primary, modifier = Modifier.weight(1f)) } }
    if (c.optBoolean("timed")) CardRow("bell") {
      Box(Modifier.weight(1f)) {
        AlertPick(c.arr("alertOptions"), c.optString("alert"), plain = true) { v ->
          model.ownAlert(c.optString("answerKey"), v)
          model.card = JSONObject(c.toString()).put("alert", v)
        }
      }
    }
    val people = c.arr("people")
    if (people.isNotEmpty()) {
      CardRow("people", Modifier.clickable { peopleOpen = !peopleOpen }, last = peopleOpen) {
        Text(c.optString("peopleSummary"), fontSize = 15.sp, modifier = Modifier.weight(1f))
        Icon24("chevron", Modifier.rotate(if (peopleOpen) 90f else 0f), size = 16.dp, tint = Tokens.text3)
      }
      if (peopleOpen) {
        Column(Modifier.padding(start = 32.dp, top = 4.dp, bottom = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
          people.forEach { p ->
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
              val (mark, col) = when (p.text("answer")) { "yes" -> "✓" to Colors.of("green"); "maybe" -> "?" to Colors.of("orange"); "no" -> "✕" to Colors.of("red"); else -> "" to Tokens.text3 }
              Box(Modifier.size(18.dp).clip(CircleShape).then(if (mark.isEmpty()) Modifier.border(1.5.dp, col, CircleShape) else Modifier.background(col.copy(alpha = 0.22f))), contentAlignment = Alignment.Center) {
                Text(mark, fontSize = 11.sp, fontWeight = FontWeight.Bold, color = col)
              }
              Text(p.text("name")?.takeIf { it.isNotEmpty() } ?: p.optString("email"), fontSize = 14.sp, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
              if (p.optBoolean("organizer")) Text("organizer", fontSize = 13.sp, color = Tokens.text2)
              if (p.optBoolean("you")) Text("you", fontSize = 13.sp, color = Tokens.text2)
            }
          }
        }
        Box(Modifier.fillMaxWidth().height(1.dp).background(Tokens.line))
      }
    }
    if (c.optBoolean("hasNotes")) LinkedText(c.arr("notes"), Modifier.padding(top = 14.dp))
    if (c.optBoolean("hasJoining")) {
      Row(Modifier.fillMaxWidth().heightIn(min = 38.dp).clickable { joiningOpen = !joiningOpen }, verticalAlignment = Alignment.CenterVertically) {
        Text("Joining details", fontSize = 14.sp, color = Tokens.text2); Spacer(Modifier.width(6.dp))
        Icon24("chevron", Modifier.rotate(if (joiningOpen) 90f else 0f), size = 15.dp, tint = Tokens.text3)
      }
      if (joiningOpen) LinkedText(c.arr("joining"), Modifier.padding(top = 2.dp), small = true)
    }
    if (c.optBoolean("asksAnswer")) {
      val opts = listOf("yes", "maybe", "no")
      val current = opts.indexOf(c.text("answer") ?: "")
      Row(Modifier.fillMaxWidth().padding(top = 20.dp).clip(RoundedCornerShape(10.dp)).background(Tokens.surface2).padding(3.dp), horizontalArrangement = Arrangement.spacedBy(2.dp)) {
        listOf("Going", "Maybe", "No").forEachIndexed { i, label ->
          val on = i == current
          Box(Modifier.weight(1f).shadow(if (on) 1.5.dp else 0.dp, RoundedCornerShape(8.dp)).clip(RoundedCornerShape(8.dp)).background(if (on) MaterialTheme.colorScheme.surface else Color.Transparent).clickable { model.answer(c.optString("answerKey"), opts[i]) }.padding(vertical = 9.dp), contentAlignment = Alignment.Center) {
            Text(label, fontSize = 15.sp, fontWeight = FontWeight.Medium, color = if (on) MaterialTheme.colorScheme.onSurface else Tokens.text2)
          }
        }
      }
    }
  }
}

@Composable
private fun LinkedText(parts: List<JSONObject>, modifier: Modifier, small: Boolean = false) {
  val link = MaterialTheme.colorScheme.primary
  val text = buildAnnotatedString {
    for (p in parts) {
      val href = p.text("href")
      if (href != null) withLink(LinkAnnotation.Url(href)) { withStyle(SpanStyle(color = link)) { append(p.optString("text")) } } else append(p.optString("text"))
    }
  }
  Text(text, modifier, fontSize = if (small) 13.sp else 15.sp, lineHeight = if (small) 19.sp else 22.sp, color = if (small) Tokens.text2 else MaterialTheme.colorScheme.onSurface)
}

/** A date in a field, picked from a calendar. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun DateField(value: String, modifier: Modifier = Modifier, plain: Boolean = false, onChange: (String) -> Unit) {
  var picking by remember { mutableStateOf(false) }
  if (plain) {
    // In a row of the edit window: just the date, tapped to change it.
    Text(friendlyDate(value), modifier.clip(RoundedCornerShape(8.dp)).clickable { picking = true }.padding(horizontal = 6.dp, vertical = 8.dp), fontSize = 15.sp, maxLines = 1)
  } else Box(modifier.fillMaxWidth().height(46.dp).clip(Tokens.field).background(Tokens.surface2).border(1.dp, Tokens.line, Tokens.field).clickable { picking = true }.padding(horizontal = 14.dp), contentAlignment = Alignment.CenterStart) {
    Text(friendlyDate(value), fontSize = 15.sp, maxLines = 1)
  }
  if (picking) {
    val state = rememberDatePickerState(initialSelectedDateMillis = runCatching { LocalDate.parse(value).atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli() }.getOrNull())
    DatePickerDialog(
      onDismissRequest = { picking = false },
      confirmButton = { TextButton(onClick = { picking = false; state.selectedDateMillis?.let { onChange(Instant.ofEpochMilli(it).atZone(ZoneOffset.UTC).toLocalDate().toString()) } }) { Text("OK") } },
      dismissButton = { TextButton(onClick = { picking = false }) { Text("Cancel") } },
    ) { DatePicker(state) }
  }
}

/** `.toggle`: a checkbox and its label. */
@Composable
private fun Toggle(label: String, on: Boolean, onChange: (Boolean) -> Unit) {
  Row(Modifier.clickable { onChange(!on) }, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
    Box(Modifier.size(20.dp).clip(RoundedCornerShape(5.dp)).background(if (on) MaterialTheme.colorScheme.primary else Color.Transparent).border(1.5.dp, if (on) MaterialTheme.colorScheme.primary else Tokens.text3, RoundedCornerShape(5.dp)), contentAlignment = Alignment.Center) {
      if (on) Icon24("check", size = 16.dp, stroke = 2.6f, tint = Color.White)
    }
    Text(label, fontWeight = FontWeight.Medium)
  }
}

@Composable
private fun EventSheet(model: CalendarModel, start: JSONObject) {
  var ed by remember(start) { mutableStateOf(start) }
  fun set(k: String, v: Any?) { ed = JSONObject(ed.toString()).put(k, v ?: JSONObject.NULL) }
  fun setRep(k: String, v: Any?) { ed = JSONObject(ed.toString()).also { it.getJSONObject("repeat").put(k, v ?: JSONObject.NULL) } }
  val isNew = ed.text("id").isNullOrEmpty()
  val rep = ed.getJSONObject("repeat")
  val freq = rep.optString("freq")
  val allDay = ed.optBoolean("all_day")
  val cals = model.screen.arr("calendars").filter { it.optBoolean("canEdit") || it.optString("id") == ed.optString("calendar") }
  val cal = cals.firstOrNull { it.optString("id") == ed.optString("calendar") }
  WebSheet({ model.editor = null }, padding = 22.dp) {
    // As on the card: the title big and without a box, the calendar under it, then rows with icons.
    Row(verticalAlignment = Alignment.CenterVertically) {
      WebField(ed.optString("title"), { set("title", it) }, Modifier.weight(1f), placeholder = "New event", minHeight = 48.dp, border = false, textStyle = TextStyle(fontSize = 22.sp, fontWeight = FontWeight.SemiBold))
      IconBtn("close", { model.editor = null }, size = 19.dp)
    }
    Row(Modifier.padding(start = 2.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(7.dp)) {
      Box(Modifier.size(8.dp).clip(CircleShape).background(Colors.of(cal?.optString("color"))))
      ProvideText(Tokens.text2) { Select(cals.map { it.optString("id") to it.optString("name") }, ed.optString("calendar"), plain = true, small = true) { set("calendar", it) } }
    }
    IconRow("clock") {
      Text("All-day", fontSize = 15.sp, modifier = Modifier.weight(1f))
      WebSwitch(allDay, { set("all_day", it) })
    }
    listOf(Triple("Starts", "date", "start"), Triple("Ends", "end_date", "end")).forEach { (label, dk, tk) ->
      IconRow(null) {
        Text(label, fontSize = 15.sp, color = Tokens.text2, modifier = Modifier.weight(1f))
        DateField(ed.optString(dk), plain = true) { if (dk == "date") ed = JSONObject(moveStart(ed.toString(), it)) else set("end_date", it) }
        if (!allDay) TimeField(ed.optString(tk), { t -> t?.let { set(tk, it) } }, Modifier.width(76.dp), plain = true)
      }
    }
    val freqs = mutableListOf("NONE" to "Never", "DAILY" to "Every day", "WEEKLY" to "Every week", "MONTHLY" to "Every month", "YEARLY" to "Every year")
    if (rep.has("raw")) freqs += "CUSTOM" to rep.optString("label")
    IconRow("repeat") { Select(freqs, freq, plain = true) { setRep("freq", it) }; Spacer(Modifier.weight(1f)) }
    if (freq == "WEEKLY") {
      val days = rep.strs("days")
      IconRow(null) {
        Row(Modifier.weight(1f), horizontalArrangement = Arrangement.SpaceBetween) {
          listOf("MO" to "M", "TU" to "T", "WE" to "W", "TH" to "T", "FR" to "F", "SA" to "S", "SU" to "S").forEach { (code, letter) ->
            val on = code in days
            Box(Modifier.size(34.dp).clip(CircleShape).background(if (on) MaterialTheme.colorScheme.primary else Tokens.surface2).clickable {
              val next = if (on) days - code else days + code
              if (next.isNotEmpty()) setRep("days", JSONArray(next))
            }, contentAlignment = Alignment.Center) { Text(letter, color = if (on) Color.White else Tokens.text2, fontWeight = FontWeight.Bold, fontSize = 14.sp) }
          }
        }
      }
    }
    if (freq != "NONE" && freq != "CUSTOM") {
      val n = rep.optInt("interval", 1)
      val unit = (mapOf("DAILY" to "day", "WEEKLY" to "week", "MONTHLY" to "month", "YEARLY" to "year")[freq] ?: "") + if (n > 1) "s" else ""
      IconRow(null) {
        Text("Every", fontSize = 15.sp, color = Tokens.text2)
        WebField(n.toString(), { v -> v.toIntOrNull()?.let { setRep("interval", it.coerceIn(1, 99)) } }, Modifier.width(52.dp), minHeight = 36.dp, textStyle = TextStyle(fontSize = 15.sp, textAlign = TextAlign.Center), keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number))
        Text(unit, fontSize = 15.sp, color = Tokens.text2, modifier = Modifier.weight(1f))
        Select(listOf("never" to "forever", "until" to "until a date", "count" to "a number of times"), rep.optString("ends"), plain = true) { setRep("ends", it) }
      }
      when (rep.optString("ends")) {
        "until" -> IconRow(null) { Text("Until", fontSize = 15.sp, color = Tokens.text2, modifier = Modifier.weight(1f)); DateField(rep.optString("until"), plain = true) { setRep("until", it) } }
        "count" -> IconRow(null) {
          WebField(rep.optInt("count", 10).toString(), { v -> v.toIntOrNull()?.let { setRep("count", it.coerceIn(1, 999)) } }, Modifier.width(60.dp), minHeight = 36.dp, textStyle = TextStyle(fontSize = 15.sp, textAlign = TextAlign.Center), keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number))
          Text("times", fontSize = 15.sp, color = Tokens.text2, modifier = Modifier.weight(1f))
        }
      }
    }
    if (!allDay) {
      val before = model.screen.opt("before").takeIf { it is Number }?.let { (it as Number).toLong() }
      IconRow("bell") { AlertPick(objects(alertOptions(ed.optString("alert"), before, true)), ed.optString("alert"), plain = true) { set("alert", it) }; Spacer(Modifier.weight(1f)) }
    }
    IconRow("place") { WebField(ed.optString("location"), { set("location", it) }, Modifier.weight(1f), placeholder = "Location", minHeight = 36.dp, border = false, textStyle = TextStyle(fontSize = 15.sp)) }
    IconRow("notes", last = true, top = true) { WebField(ed.optString("notes"), { set("notes", it) }, Modifier.weight(1f), placeholder = "Notes", singleLine = false, minHeight = 60.dp, border = false, textStyle = TextStyle(fontSize = 15.sp)) }
    Row(Modifier.fillMaxWidth().padding(top = 18.dp), horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
      if (!isNew) Btn("Delete", { model.delete(ed, null) }, kind = BtnKind.Danger)
      Spacer(Modifier.weight(1f))
      if (isNew) Btn("Cancel", { model.editor = null })
      Btn(if (isNew) "Add" else "Save", { model.save(ed, null) }, kind = BtnKind.Primary)
    }
  }
}

@Composable
private fun ScopeSheet(model: CalendarModel, ed: JSONObject, ask: String) {
  val verb = if (ask == "save") "Change" else "Delete"
  val delete = ask == "delete"
  WebSheet({ model.scopeAsk = null }) {
    Text("$verb a repeating event", fontSize = 22.sp, fontWeight = FontWeight.SemiBold, letterSpacing = (-0.4).sp)
    Text("$verb only this one, on ${friendlyDate(ed.optString("occurrence"))}, or every time it repeats?", color = Tokens.text2, modifier = Modifier.padding(top = 10.dp))
    Row(Modifier.fillMaxWidth().padding(top = 22.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
      Btn("Only this one", { if (delete) model.delete(ed, "this") else model.save(ed, "this") }, Modifier.weight(1f), kind = if (delete) BtnKind.Danger else BtnKind.Normal, lg = true)
      Btn("All of them", { if (delete) model.delete(ed, "all") else model.save(ed, "all") }, Modifier.weight(1f), kind = if (delete) BtnKind.Danger else BtnKind.Primary, lg = true)
    }
    Btn("Cancel", { model.scopeAsk = null }, Modifier.fillMaxWidth().padding(top = 8.dp), kind = BtnKind.Ghost)
  }
}

/** `.share-row`: a grey pill as wide as the sheet, blue text and icon. */
@Composable
private fun ShareRow(icon: String, text: String, onClick: () -> Unit) {
  Row(Modifier.fillMaxWidth().padding(top = 18.dp).height(46.dp).clip(Pill.shape).background(Tokens.surface2).clickable(onClick = onClick).padding(horizontal = 18.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
    Icon24(icon, size = 18.dp, tint = MaterialTheme.colorScheme.primary)
    Text(text, color = MaterialTheme.colorScheme.primary, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
  }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun CalSheet(model: CalendarModel, c: JSONObject, export: (String) -> Unit) {
  var name by remember(c) { mutableStateOf(c.optString("name")) }
  var color by remember(c) { mutableStateOf(c.optString("color", "blue")) }
  val id = c.text("id")
  val from = c.optJSONObject("from")
  WebSheet({ model.calSheet = null }) {
    Text(if (from != null) c.optString("name") else if (id != null) "Edit calendar" else "New calendar", fontSize = 22.sp, fontWeight = FontWeight.SemiBold, letterSpacing = (-0.4).sp)
    if (from != null) {
      Text("Shared by ${from.optString("name")}" + if (c.optBoolean("canWrite")) "" else " · view only", color = Tokens.text2, modifier = Modifier.padding(top = 10.dp))
    } else {
      WebField(name, { name = it }, Modifier.padding(top = 16.dp), placeholder = "Name, e.g. Work")
      FlowRow(Modifier.fillMaxWidth().padding(top = 18.dp, bottom = 6.dp), horizontalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterHorizontally), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        COLORS.forEach { k ->
          Box(Modifier.size(32.dp).then(if (k == color) Modifier.border(2.dp, Tokens.text2, CircleShape).padding(5.dp) else Modifier).clip(CircleShape).background(Colors.of(k)).clickable { color = k })
        }
      }
    }
    if (c.optBoolean("subscribed")) {
      val sub = model.data.arr("subs").firstOrNull { it.optString("id") == id }
      if (sub != null) Text("Updated ${ago(sub.optLong("refreshed"))}", fontSize = 13.sp, color = Tokens.text2, modifier = Modifier.padding(top = 16.dp))
      if (sub != null) ShareRow("download", "Update now") { model.refreshNow(id!!) }
    }
    if (id != null) ShareRow("share", "Save as .ics file") { export(id) }
    if (model.note.isNotEmpty()) Text(model.note, fontSize = 13.sp, color = Tokens.text2, modifier = Modifier.padding(top = 16.dp))
    if (c.optBoolean("main")) {
    } else if (id != null && from == null) {
      val shared = (c.optJSONArray("sharedWith")?.length() ?: 0) > 0
      ShareRow("share", if (shared) c.optString("note") else "Share calendar…") { model.share(id) }
    }
    Row(Modifier.fillMaxWidth().padding(top = 22.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
      when {
        from != null -> Btn("Leave calendar", { model.deleteCal(id!!, false) }, Modifier.weight(1f), kind = BtnKind.Danger, lg = true)
        id != null && !c.optBoolean("main") -> Btn("Delete", { model.deleteCal(id, c.optBoolean("subscribed")) }, Modifier.weight(1f), kind = BtnKind.Danger, lg = true)
        else -> Btn("Cancel", { model.calSheet = null }, Modifier.weight(1f), lg = true)
      }
      if (from != null) Btn("Done", { model.calSheet = null }, Modifier.weight(1f), kind = BtnKind.Primary, lg = true)
      else Btn("Save", { model.saveCal(JSONObject(c.toString()).put("name", name).put("color", color)) }, Modifier.weight(1f), kind = BtnKind.Primary, lg = true, enabled = name.isNotBlank())
    }
  }
}

@Composable
private fun SubscribeSheet(model: CalendarModel) {
  var url by remember { mutableStateOf("") }
  var name by remember { mutableStateOf("") }
  WebSheet({ model.subscribing = false }) {
    Text("Subscribe to a link", fontSize = 22.sp, fontWeight = FontWeight.SemiBold, letterSpacing = (-0.4).sp)
    Column(Modifier.padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
      WebField(url, { url = it }, placeholder = "https://… or webcal://…", keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri))
      WebField(name, { name = it }, placeholder = "Name (optional)")
    }
    Row(Modifier.fillMaxWidth().padding(top = 22.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
      Btn("Cancel", { model.subscribing = false }, Modifier.weight(1f), lg = true)
      Btn("Subscribe", { model.subscribeTo(url, name) }, Modifier.weight(1f), kind = BtnKind.Primary, lg = true, enabled = url.isNotBlank())
    }
  }
}

@Composable
private fun ImportSheet(model: CalendarModel, items: List<JSONObject>) {
  var targets by remember(items) { mutableStateOf(items.map { "new" }) }
  val cals = model.screen.arr("calendars").filter { it.optBoolean("canEdit") }
  WebSheet({ model.importItems = null }) {
    Text("Import", fontSize = 22.sp, fontWeight = FontWeight.SemiBold, letterSpacing = (-0.4).sp)
    items.forEachIndexed { i, it ->
      Column(Modifier.fillMaxWidth().padding(vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
          Text(it.optString("name"), fontWeight = FontWeight.Bold)
          val err = it.text("error")
          if (err != null) Text(err, fontSize = 13.sp, color = Tokens.text2)
          else {
            val rep = it.optInt("repeating")
            Text("${it.optInt("events")} events${if (rep > 0) ", $rep repeating" else ""}", fontSize = 13.sp, color = Tokens.text2)
          }
        }
        if (it.text("error") == null) Select(listOf("new" to "New calendar") + cals.map { c -> c.optString("id") to "Into ${c.optString("name")}" }, targets[i]) { v -> targets = targets.toMutableList().also { t -> t[i] = v } }
      }
      Box(Modifier.fillMaxWidth().height(1.dp).background(Tokens.line))
    }
    Row(Modifier.fillMaxWidth().padding(top = 22.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
      Btn("Cancel", { model.importItems = null }, Modifier.weight(1f), lg = true)
      Btn("Import", { model.runImport(items.mapIndexed { i, it -> JSONObject(it.toString()).put("target", targets[i]) }) }, Modifier.weight(1f), kind = BtnKind.Primary, lg = true, enabled = items.any { it.text("error") == null })
    }
  }
}

@Composable
private fun CalendarSettings(model: CalendarModel, s: JSONObject) {
  val key = if (model.narrow) "narrow" else "wide"
  val sw = s.getJSONObject("switch").getJSONObject(key)
  val three = sw.optBoolean("three")
  SettingsSheet(onClose = { model.settingsOpen = false }, week = true) {
    SettingsGroup("Views") {
      val on = s.arr("views").map { it.optString("id") }
      val most = if (model.narrow) 5 else 7
      VIEWS.forEachIndexed { i, (id, name) ->
        val ticked = id in on
        val can = if (ticked) on.size > 1 else on.size < most
        IconRow(if (i == 0) "calendar" else null, Modifier.clickable(enabled = can) { model.setViews(if (ticked) on - id else on + id) }, last = i == VIEWS.size - 1) {
          Text(name, fontSize = 15.sp, modifier = Modifier.weight(1f))
          Check(ticked, null, { if (can) model.setViews(if (ticked) on - id else on + id) }, enabled = can || ticked)
        }
      }
    }
    SettingsGroup("Events") {
      val before = s.opt("before").takeIf { it is Number }?.let { (it as Number).toLong() }
      val current = before?.toString() ?: "none"
      IconRow("bell") {
        Text("Default alert", fontSize = 15.sp, modifier = Modifier.weight(1f))
        AlertPick(objects(alertOptions(current, before, false)), current, plain = true) { model.setBefore(it.toLongOrNull()) }
      }
      val emails = s.strs("emails")
      IconRow("at", last = emails.isEmpty()) {
        Text("Your addresses", fontSize = 15.sp, modifier = Modifier.weight(1f))
        if (emails.isEmpty()) Text("None yet", fontSize = 15.sp, color = Tokens.text3)
      }
      emails.forEachIndexed { i, a -> IconRow(null, last = i == emails.size - 1) { Text(a, fontSize = 15.sp, color = Tokens.text2, modifier = Modifier.weight(1f), maxLines = 1); IconBtn("close", { model.forgetAddress(a) }, size = 16.dp) } }
    }
  }
}
