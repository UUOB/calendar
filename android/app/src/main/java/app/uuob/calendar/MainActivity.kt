package app.uuob.calendar

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CalendarMonth
import androidx.compose.runtime.remember
import androidx.lifecycle.lifecycleScope
import app.uuob.kit.AppInfo
import app.uuob.kit.AppTheme
import app.uuob.kit.Colors
import app.uuob.kit.Gate
import app.uuob.kit.Updater
import app.uuob.kit.Uuob
import app.uuob.kit.UuobAlerts

/** Who Calendar is, for UUOB and its connect screen. */
val calendarApp = AppInfo(
  id = "calendar",
  name = "Calendar",
  noun = "events",
  icon = Icons.Rounded.CalendarMonth,
  color = Colors.of("red"),
  version = BuildConfig.VERSION_NAME,
  updateFeed = "https://raw.githubusercontent.com/jachymri/uuob-releases/main/calendar.json",
  updateKey = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDU5QjY3MTNBQjA5NTI0QzAKUldUQUpKV3dPbkcyV2VzZU9XSWJQT29NWTR3angxNjU1VWJZazJsMi80Zjl6WDJoSWxMK0t3MlUK",
)

class MainActivity : ComponentActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    // Alerts are Calendar's own notifications; Android 13+ shows them only once allowed.
    UuobAlerts.askPermission(this)
    Uuob.init(applicationContext, calendarApp)
    Upcoming.askIfNone(applicationContext)
    val updater = Updater(applicationContext, lifecycleScope)
    setContent {
      AppTheme(calendarApp.color) {
        val model = remember { CalendarModel(applicationContext) }
        Gate(updater) { CalendarScreen(model, updater) }
      }
    }
  }
}

/** UUOB handing over an alert when it is due, shown as Calendar's own notification (only from UUOB). */
class AlertReceiver : BroadcastReceiver() {
  override fun onReceive(ctx: Context, intent: Intent) = UuobAlerts.show(ctx, intent, R.drawable.ic_notify, MainActivity::class.java)
}
