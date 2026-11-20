//! Calendar's window on Linux: GTK 4 and Libadwaita through Relm4, on
//! Calendar's core (`core/`) and UUOB's SDK. What a view shows comes from the
//! core (`app::screen`); this draws it and sends the user's changes back.
//! The connect screen, Settings and the time field are UUOB's kit.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use calendar_core::{app, zones};
use relm4::adw::prelude::*;
use relm4::{ComponentParts, ComponentSender, RelmApp, adw, gtk, gtk::gdk, gtk::gio, gtk::glib};
use serde_json::{Value, json};
use uuob_client::{App, Client, Event, dates};
use uuob_kit_gtk::{AppInfo, Gate};

const FEED: &str = "https://raw.githubusercontent.com/jachymri/uuob-releases/main/calendar.json";
/// Pixels per hour in the grid.
const HOUR: f64 = 52.0;

fn data_dir() -> PathBuf {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    std::env::var_os("XDG_DATA_HOME").map(Into::into).unwrap_or_else(|| home.join(".local/share")).join("app.uuob.calendar")
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or_default()
}
fn arr(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}
fn strs(v: &Value) -> Vec<String> {
    arr(v).iter().filter_map(|x| x.as_str().map(String::from)).collect()
}
fn rgb(name: &str) -> (f64, f64, f64) {
    uuob_kit_gtk::color(name)
}

#[derive(Debug)]
pub enum Msg {
    Reload,
    Redraw,
    Tick,
    Show(String),
    Move(i64),
    Today,
    Day(String),
    NewEvent(String, Option<String>, Option<String>),
    Open(String),
    Edit(String),
    Save(Value, Option<String>),
    Delete(Value, Option<String>),
    Answer(String, String),
    OwnAlert(String, String),
    ToggleCal(String),
    ToggleTasks,
    CalSheet(Option<Value>),
    SaveCal(Value),
    DeleteCal(String, bool),
    Share(String),
    Export(String),
    RefreshNow(String),
    Subscribe(String, String),
    Import,
    ImportPick(Vec<Value>),
    ImportRun(Vec<Value>),
    Settings,
    SetSwitch(String, Value),
    SetBefore(Option<i64>),
    ForgetAddress(String),
    Note(String),
    Upcoming(u64),
}

#[derive(Debug)]
pub enum Cmd {
    Loaded(Box<Value>),
    Tasks(Vec<Value>),
    Done,
    Error(String),
    SubResult(Result<String, String>),
    ImportResult(Value),
}

pub struct Calendar {
    client: Client,
    info: AppInfo,
    data: Value,
    view: String,
    cursor: String,
    narrow: bool,
    hidden: Option<Vec<String>>,
    show_tasks: Option<bool>,
    views: Option<Value>,
    tasks: Vec<Value>,
    screen: Value,
    events_known: bool,
    prefs_known: bool,
    refreshed_once: bool,
    upcoming_gen: u64,
    shown_today: String,
}

pub struct Widgets {
    root: adw::ApplicationWindow,
    cals: gtk::ListBox,
    tasks_check: gtk::CheckButton,
    title: gtk::Label,
    switch: gtk::Box,
    body: gtk::Stack,
    heads: gtk::Box,
    allday: gtk::Box,
    columns: gtk::Box,
    hours: gtk::DrawingArea,
    grid_scroll: gtk::ScrolledWindow,
    month: gtk::Grid,
    toast: adw::ToastOverlay,
}

/// A pill's text colour: the calendar colour mixed 30% toward the text (`.pill`).
fn tint_rgb(name: &str) -> (f64, f64, f64) {
    let (r, g, b) = rgb(name);
    let t = if adw::StyleManager::default().is_dark() { 0.96 } else { 0.11 };
    (r * 0.7 + t * 0.3, g * 0.7 + t * 0.3, b * 0.7 + t * 0.3)
}

fn tint_hex(name: &str) -> String {
    let (r, g, b) = tint_rgb(name);
    format!("#{:02x}{:02x}{:02x}", (r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// The page colour (`--bg`), for what the event blocks mix with.
fn page_rgb() -> (f64, f64, f64) {
    if adw::StyleManager::default().is_dark() { (0.043, 0.043, 0.051) } else { (0.961, 0.961, 0.969) }
}

/// A task with a date, from Tasks: a hollow tick and its title.
fn task_line(title: &str, classes: &[&str]) -> gtk::Box {
    let b = gtk::Box::builder().spacing(6).css_classes(classes.to_vec()).build();
    b.append(&gtk::Box::builder().valign(gtk::Align::Center).css_classes(["tick"]).build());
    b.append(&gtk::Label::builder().label(title).xalign(0.0).hexpand(true).ellipsize(gtk::pango::EllipsizeMode::End).build());
    b
}

fn answer_class(w: &impl IsA<gtk::Widget>, answer: &Value) {
    if let Some(a) = answer.as_str() {
        w.add_css_class(&format!("answer-{a}"));
    }
}

/// Empties a box, list or grid (each takes its children out its own way).
fn clear(b: &impl IsA<gtk::Widget>) {
    let w = b.as_ref();
    if let Some(l) = w.downcast_ref::<gtk::ListBox>() {
        l.remove_all();
    } else if let Some(g) = w.downcast_ref::<gtk::Grid>() {
        while let Some(c) = g.first_child() {
            g.remove(&c);
        }
    } else if let Some(x) = w.downcast_ref::<gtk::Box>() {
        while let Some(c) = x.first_child() {
            x.remove(&c);
        }
    }
}

/// Where an event's block is in a column `wd` wide: x, y, width, height.
fn block_rect(b: &Value, wd: f64) -> (f64, f64, f64, f64) {
    let (st, en) = (b["start"].as_f64().unwrap_or(0.0), b["end"].as_f64().unwrap_or(0.0));
    let (lane, lanes) = (b["lane"].as_f64().unwrap_or(0.0), b["lanes"].as_f64().unwrap_or(1.0).max(1.0));
    let h = ((en - st) / 60.0 * HOUR - 2.0).max(22.0);
    (lane / lanes * wd + 2.0, st / 60.0 * HOUR + 1.0, wd / lanes - 4.0, h)
}

fn rounded(cr: &gtk::cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    use std::f64::consts::PI;
    let r = r.min(h / 2.0).min(w / 2.0).max(0.0);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -PI / 2.0, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, PI / 2.0);
    cr.arc(x + r, y + h - r, r, PI / 2.0, PI);
    cr.arc(x + r, y + r, r, PI, 1.5 * PI);
    cr.close_path();
}

/// One day of the hour grid, drawn: hour lines, the events side by side, the
/// tasks with a time, the line for now. A click opens an event; dragging on
/// an empty part makes one (a finger rests first, so scrolling never does).
fn day_column(col: Value, now_minute: i64, hour12: bool, sender: &ComponentSender<Calendar>) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder().hexpand(true).content_height((HOUR * 24.0) as i32).build();
    let drag: Rc<RefCell<Option<(i64, i64)>>> = Rc::new(RefCell::new(None));
    let blocks = arr(&col["timed"]);
    let today = col["today"] == json!(true);
    {
        let (drag, blocks, col) = (drag.clone(), blocks.clone(), col.clone());
        area.set_draw_func(move |a, cr, wd, ht| {
            let wd = wd as f64;
            let fg = a.color();
            let (fr, fgc, fb) = (fg.red() as f64, fg.green() as f64, fg.blue() as f64);
            if today {
                cr.set_source_rgba(0.04, 0.52, 1.0, 0.03);
                cr.rectangle(0.0, 0.0, wd, ht as f64);
                let _ = cr.fill();
            }
            cr.set_source_rgba(fr, fgc, fb, 0.08);
            cr.set_line_width(1.0);
            for h in 0..24 {
                cr.move_to(0.0, h as f64 * HOUR + 0.5);
                cr.line_to(wd, h as f64 * HOUR + 0.5);
            }
            cr.move_to(0.5, 0.0);
            cr.line_to(0.5, ht as f64);
            let _ = cr.stroke();
            let layout = a.create_pango_layout(None);
            layout.set_wrap(gtk::pango::WrapMode::WordChar);
            for b in &blocks {
                let (x, y, bw, bh) = block_rect(b, wd);
                let (r, g, bl) = rgb(s(b, "color"));
                let answer = s(b, "answer");
                let declined = answer == "no";
                let (pr, pg, pb) = page_rgb();
                let mixed = |a: f64| (r * a + pr * (1.0 - a), g * a + pg * (1.0 - a), bl * a + pb * (1.0 - a));
                cr.save().ok();
                rounded(cr, x, y, bw, bh, 7.0);
                cr.clip();
                if declined {
                    cr.push_group();
                }
                // Not answered yet: an outline; maybe: stripes; otherwise a tint (`.event`).
                match answer {
                    "invited" => {
                        cr.set_source_rgb(pr, pg, pb);
                        cr.paint().ok();
                        cr.set_source_rgba(r, g, bl, 0.7);
                        rounded(cr, x + 0.5, y + 0.5, bw - 1.0, bh - 1.0, 7.0);
                        let _ = cr.stroke();
                    }
                    "maybe" => {
                        let (a1, a2) = (mixed(0.26), mixed(0.12));
                        cr.set_source_rgb(a2.0, a2.1, a2.2);
                        cr.paint().ok();
                        cr.set_source_rgb(a1.0, a1.1, a1.2);
                        cr.set_line_width(6.0);
                        let mut k = -bh;
                        while k < bw + bh {
                            cr.move_to(x + k, y + bh);
                            cr.line_to(x + k + bh, y);
                            k += 17.0;
                        }
                        let _ = cr.stroke();
                        cr.set_line_width(1.0);
                    }
                    _ => {
                        let m = mixed(0.24);
                        cr.set_source_rgb(m.0, m.1, m.2);
                        cr.paint().ok();
                    }
                }
                cr.set_source_rgb(r, g, bl);
                cr.rectangle(x, y, 3.0, bh);
                let _ = cr.fill();
                // The title wraps over the lines the block has room for; the time goes
                // below when a line is left for it, then the place if there's room (as on the web).
                let strike = |t: String| if declined { format!("<s>{t}</s>") } else { t };
                let width = ((bw - 13.0).max(10.0) * gtk::pango::SCALE as f64) as i32;
                cr.set_source_rgb(r * 0.55 + fr * 0.45, g * 0.55 + fgc * 0.45, bl * 0.55 + fb * 0.45);
                let title = glib::markup_escape_text(s(b, "title"));
                layout.set_width(width);
                if bh < 38.0 {
                    layout.set_height(-1);
                    layout.set_ellipsize(gtk::pango::EllipsizeMode::End);
                    layout.set_markup(&strike(format!("<span size=\"9750\" weight=\"650\">{title}</span> <span size=\"9000\" alpha=\"85%\">{}</span>", glib::markup_escape_text(s(b, "startTime")))));
                    cr.move_to(x + 9.0, y + (bh - layout.pixel_size().1 as f64) / 2.0);
                    pangocairo::functions::show_layout(cr, &layout);
                } else {
                    let place = if bh >= 76.0 { s(b, "place") } else { "" };
                    let room = (((bh - 5.0 - if place.is_empty() { 0.0 } else { 15.0 }) / 15.0).floor() as i32).max(1);
                    layout.set_markup(&strike(format!("<span size=\"9750\" weight=\"650\">{title}</span>")));
                    layout.set_ellipsize(gtk::pango::EllipsizeMode::None);
                    layout.set_height(-1);
                    let needs = layout.line_count();
                    let time_line = needs < room;
                    let lines = if time_line { room - 1 } else { room };
                    layout.set_ellipsize(gtk::pango::EllipsizeMode::End);
                    layout.set_height(-lines);
                    let mut ty = y + 3.0;
                    cr.move_to(x + 9.0, ty);
                    pangocairo::functions::show_layout(cr, &layout);
                    ty += layout.pixel_size().1 as f64;
                    layout.set_ellipsize(gtk::pango::EllipsizeMode::End);
                    layout.set_height(-1);
                    let small = |t: &str| format!("<span size=\"9000\" alpha=\"85%\">{}</span>", glib::markup_escape_text(t));
                    if time_line {
                        layout.set_markup(&small(s(b, "time")));
                        cr.move_to(x + 9.0, ty);
                        pangocairo::functions::show_layout(cr, &layout);
                        ty += layout.pixel_size().1 as f64;
                    }
                    if !place.is_empty() {
                        layout.set_markup(&small(place));
                        cr.move_to(x + 9.0, ty);
                        pangocairo::functions::show_layout(cr, &layout);
                    }
                }
                layout.set_ellipsize(gtk::pango::EllipsizeMode::None);
                layout.set_height(-1);
                if declined {
                    cr.pop_group_to_source().ok();
                    cr.paint_with_alpha(0.45).ok();
                }
                cr.restore().ok();
            }
            layout.set_width(-1);
            // `.task-mark`: a grey pill with a hollow tick at the task's time.
            for t in arr(&col["tasksTimed"]) {
                let y = t["minute"].as_f64().unwrap_or(0.0) / 60.0 * HOUR;
                let dark = adw::StyleManager::default().is_dark();
                let s2 = if dark { (0.149, 0.149, 0.165) } else { (0.941, 0.941, 0.953) };
                cr.set_source_rgb(s2.0, s2.1, s2.2);
                rounded(cr, 2.0, y, wd - 4.0, 19.0, 6.0);
                let _ = cr.fill();
                cr.set_source_rgba(fr, fgc, fb, 0.38);
                cr.set_line_width(1.5);
                cr.arc(13.0, y + 9.5, 4.25, 0.0, std::f64::consts::TAU);
                let _ = cr.stroke();
                cr.set_line_width(1.0);
                layout.set_markup(&format!("<span size=\"9000\">{}</span>", glib::markup_escape_text(s(&t, "title"))));
                layout.set_width(((wd - 30.0).max(10.0) * gtk::pango::SCALE as f64) as i32);
                layout.set_ellipsize(gtk::pango::EllipsizeMode::End);
                cr.set_source_rgba(fr, fgc, fb, 0.62);
                cr.move_to(23.0, y + (19.0 - layout.pixel_size().1 as f64) / 2.0);
                pangocairo::functions::show_layout(cr, &layout);
                layout.set_ellipsize(gtk::pango::EllipsizeMode::None);
                layout.set_width(-1);
            }
            if let Some((from, to)) = *drag.borrow() {
                let (y0, y1) = (from as f64 / 60.0 * HOUR, to as f64 / 60.0 * HOUR);
                cr.set_source_rgba(0.04, 0.52, 1.0, 0.25);
                rounded(cr, 2.0, y0, wd - 4.0, y1 - y0, 5.0);
                let _ = cr.fill();
                let hm = |m: i64| dates::friendly_time(&format!("{:02}:{:02}", m.min(1439) / 60, m.min(1439) % 60), hour12);
                cr.set_source_rgba(fr, fgc, fb, 0.9);
                layout.set_markup(&format!("<small>{} – {}</small>", hm(from), hm(to)));
                cr.move_to(8.0, y0 + 2.0);
                pangocairo::functions::show_layout(cr, &layout);
            }
            if today {
                let y = now_minute as f64 / 60.0 * HOUR;
                cr.set_source_rgb(1.0, 0.27, 0.23);
                cr.set_line_width(1.5);
                cr.move_to(0.0, y);
                cr.line_to(wd, y);
                let _ = cr.stroke();
                cr.arc(3.0, y, 4.0, 0.0, std::f64::consts::TAU);
                let _ = cr.fill();
            }
        });
    }
    let snap = |y: f64| ((y / HOUR * 4.0).round() as i64 * 15).clamp(0, 1440);
    let hit = |blocks: &[Value], wd: f64, x: f64, y: f64| {
        blocks.iter().rev().find(|b| {
            let (bx, by, bw, bh) = block_rect(b, wd);
            x >= bx && x <= bx + bw && y >= by && y <= by + bh
        }).cloned()
    };
    // A click on an event opens it.
    let click = gtk::GestureClick::new();
    {
        let (tx, blocks, a) = (sender.clone(), blocks.clone(), area.clone());
        click.connect_released(move |_, _, x, y| {
            if let Some(b) = hit(&blocks, a.width() as f64, x, y) {
                tx.input(Msg::Open(s(&b, "key").to_string()));
            }
        });
    }
    area.add_controller(click);
    // Dragging on an empty part makes an event.
    let gd = gtk::GestureDrag::new();
    let held = Rc::new(RefCell::new(false));
    {
        let (drag, blocks, a, held) = (drag.clone(), blocks.clone(), area.clone(), held.clone());
        gd.connect_drag_begin(move |g, x, y| {
            if hit(&blocks, a.width() as f64, x, y).is_some() {
                g.set_state(gtk::EventSequenceState::Denied);
                return;
            }
            let touch = g.device().is_some_and(|d| d.source() == gdk::InputSource::Touchscreen);
            let m = snap(y).min(1425);
            if touch {
                *held.borrow_mut() = false;
                let (drag, a, held) = (drag.clone(), a.clone(), held.clone());
                glib::timeout_add_local_once(std::time::Duration::from_millis(350), move || {
                    if !*held.borrow() {
                        *held.borrow_mut() = true;
                        *drag.borrow_mut() = Some((m, m + 60));
                        a.queue_draw();
                    }
                });
            } else {
                *held.borrow_mut() = true;
                *drag.borrow_mut() = Some((m, m + 60));
                a.queue_draw();
            }
        });
    }
    {
        let (drag, a, held) = (drag.clone(), area.clone(), held.clone());
        gd.connect_drag_update(move |g, dx, dy| {
            if !*held.borrow() || drag.borrow().is_none() {
                if dx.hypot(dy) > 8.0 {
                    // Moved before the hold: scrolling.
                    *held.borrow_mut() = true;
                    g.set_state(gtk::EventSequenceState::Denied);
                }
                return;
            }
            let Some((_, sy)) = g.start_point() else { return };
            let from = drag.borrow().map(|d| d.0).unwrap_or(0);
            *drag.borrow_mut() = Some((from, snap(sy + dy).max(from + 15).min(1440)));
            a.queue_draw();
        });
    }
    {
        let (drag, tx, a) = (drag.clone(), sender.clone(), area.clone());
        let date = s(&col, "date").to_string();
        gd.connect_drag_end(move |_, _, _| {
            let Some((from, to)) = drag.borrow_mut().take() else { return };
            a.queue_draw();
            let hm = |m: i64| format!("{:02}:{:02}", m / 60, m % 60);
            tx.input(Msg::NewEvent(date.clone(), Some(hm(from)), Some(hm(to.min(1439)))));
        });
    }
    area.add_controller(gd);
    area
}

fn style() {
    let css = gtk::CssProvider::new();
    let mut checks = String::new();
    for c in uuob_kit_gtk::COLORS {
        // The palette's colour (light or dark as the theme is).
        let col = format!("@uuob_{c}");
        checks.push_str(&format!(".uuob-check-{c} check:checked {{ background: {col}; border-color: {col}; color: white; }} .uuob-check-{c} check {{ border-color: {col}; }}\n"));
        checks.push_str(&format!(".tint-{c} {{ background: alpha({col}, 0.22); color: mix({col}, @window_fg_color, 0.3); }} .tint-{c}.answer-invited {{ background: @window_bg_color; box-shadow: inset 0 0 0 1px alpha({col}, 0.7); }} .tint-{c}.answer-maybe {{ background: alpha({col}, 0.13); }}\n"));
    }
    css.load_from_string(&format!(
        "{checks}
         .side-col {{ padding: 6px 16px 18px; }}
         .side-label {{ margin: 0 0 4px 8px; }}
         .other-apps {{ margin-top: 28px; }}
         .cal-row {{ padding: 4px 0; }}
         list.navigation-sidebar row {{ border-radius: 10px; padding: 3px 8px; }}
         .cal-name {{ font-weight: 500; }}
         menubutton.add-cal > button {{ min-width: 30px; min-height: 30px; }}
         popover.web-menu-pop > contents {{ background: @uuob_surface2; border: 1px solid alpha(@window_fg_color, 0.14); border-radius: 14px; padding: 6px; box-shadow: 0 12px 32px rgba(0,0,0,0.2); }}
         button.web-menu-item {{ min-height: 40px; padding: 0 12px; border-radius: 9px; background: none; box-shadow: none; font-size: 15px; font-weight: 500; min-width: 186px; }}
         button.web-menu-item:hover {{ background: alpha(@window_fg_color, 0.06); }}
         .cal-row button.icon-btn.sm {{ min-width: 28px; min-height: 28px; opacity: 0; transition: opacity 150ms; }}
         row:hover .cal-row button.icon-btn.sm, .cal-row button.icon-btn.sm:focus {{ opacity: 1; }}
         check {{ border-radius: 999px; min-width: 18px; min-height: 18px; }}
         .tasks-check {{ margin-left: 8px; }}
         .tasks-check label {{ margin-left: 10px; font-weight: 500; }}
         .tasks-check check {{ border-color: @uuob_text2; }}
         .tasks-check check:checked {{ background: @uuob_text2; }}
         button.btn.settings-btn {{ color: @uuob_text2; background: none; padding: 0 10px; }}
         button.btn.settings-btn:hover {{ color: @window_fg_color; background: alpha(@window_fg_color, 0.06); }}
         .cal-main {{ padding: 0 28px 0; }}
         .cal-main.narrow {{ padding: 0 12px 0; }}
         .large-title.narrow-title {{ font-size: 28px; }}
         .cal-main.narrow .mh {{ padding: 0 0 6px; }}
         .cal-main.narrow .cell {{ min-height: 64px; }}
         .cal-header {{ margin: 0 0 18px; }}
         .cal-header .large-title {{ font-weight: 700; }}
         button.btn.new {{ min-width: 36px; min-height: 36px; padding: 0; }}
         button.head {{ padding: 6px 0 10px; background: none; box-shadow: none; }}
         .wd {{ font-size: 13px; color: @uuob_text2; font-weight: 500; letter-spacing: 0.4px; }}
         .dn {{ font-size: 20px; font-weight: 600; min-width: 34px; min-height: 34px; border-radius: 999px; }}
         button.head.today .dn {{ background: @uuob_red; color: white; }}
         button.head.today .wd {{ color: @uuob_red; }}
         separator.strong-line {{ background: alpha(@window_fg_color, 0.14); min-height: 1px; }}
         .ad-cell {{ padding: 3px 2px; min-height: 30px; border-left: 1px solid @uuob_line; }}
         .gutter {{ padding: 6px 8px 0 0; font-size: 11px; color: @uuob_text3; }}
         .ad-pill {{ padding: 3px 8px; min-height: 15px; border-radius: 6px; font-size: 12px; font-weight: 600; box-shadow: none; }}
         .tick {{ min-width: 7px; min-height: 7px; border-radius: 999px; border: 1.5px solid @uuob_text3; }}
         .task {{ background: @uuob_surface2; color: @uuob_text2; }}
         .join-host {{ opacity: 0.75; font-weight: 400; }}
         .ad-pill.cont {{ border-top-left-radius: 0; border-bottom-left-radius: 0; }}
         .ad-pill.on {{ border-top-right-radius: 0; border-bottom-right-radius: 0; }}
         .answer-no {{ opacity: 0.45; }}
         .answer-no label {{ text-decoration: line-through; }}
         .mh {{ font-size: 13px; color: @uuob_text2; font-weight: 600; padding: 0 10px 8px; letter-spacing: 0.4px; }}
         .cell {{ border-top: 1px solid @uuob_line; border-left: 1px solid @uuob_line; padding: 4px; min-height: 96px; transition: background 150ms; }}
         .cell:hover {{ background: alpha(@window_fg_color, 0.04); }}
         .cell.other .num {{ color: @uuob_text3; }}
         button.num {{ min-width: 30px; min-height: 30px; padding: 0; border-radius: 999px; font-size: 15px; font-weight: 600; }}
         button.num.today {{ background: @uuob_red; color: white; }}
         button.mev, .mev {{ padding: 2px 6px; min-height: 0; border-radius: 6px; font-size: 12px; font-weight: 550; box-shadow: none; }}
         .more {{ padding: 0 6px; font-size: 11px; color: @uuob_text2; }}
         .dot {{ border-radius: 999px; min-width: 7px; min-height: 7px; }}
         .swatch {{ min-width: 30px; min-height: 30px; padding: 0; border-radius: 999px; }}
         .swatch.on {{ box-shadow: 0 0 0 3px @window_bg_color, 0 0 0 5px @window_fg_color; }}
         .card-title {{ font-size: 22px; font-weight: 650; }}
         .stack-label {{ font-size: 13px; color: @uuob_text2; font-weight: 500; }}
         .unit {{ color: @uuob_text2; }}
         textview.field {{ min-height: 90px; }}
         button.wday {{ min-width: 38px; min-height: 38px; padding: 0; border-radius: 999px; background: @uuob_surface2; color: @uuob_text2; font-weight: 650; box-shadow: none; }}
         button.wday:checked {{ background: @accent_bg_color; color: white; }}
         .people-st {{ min-width: 18px; font-weight: 700; }}"
    ));
    gtk::style_context_add_provider_for_display(&gdk::Display::default().expect("display"), &css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
}

fn label(text: &str, classes: &[&str]) -> gtk::Label {
    gtk::Label::builder().label(text).xalign(0.0).wrap(true).wrap_mode(gtk::pango::WrapMode::WordChar).css_classes(classes.to_vec()).build()
}

/// Text with links that open in the system's app.
fn linked(parts: &Value, classes: &[&str]) -> gtk::Label {
    let mut markup = String::new();
    for p in arr(parts) {
        let t = glib::markup_escape_text(s(&p, "text"));
        match p["href"].as_str() {
            Some(h) => markup.push_str(&format!("<a href=\"{}\">{t}</a>", glib::markup_escape_text(h))),
            None => markup.push_str(&t),
        }
    }
    let l = label("", classes);
    l.set_markup(&markup);
    l
}

fn open_uri(uri: &str) {
    let _ = gio::AppInfo::launch_default_for_uri(uri, gio::AppLaunchContext::NONE);
}

/// `label.stack`: a small grey label above its field.
fn stack_box(text: &str) -> gtk::Box {
    let b = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(6).hexpand(true).build();
    b.append(&gtk::Label::builder().label(text).xalign(0.0).css_classes(["stack-label"]).build());
    b
}

/// An alert picker: the usual times, or Custom… (a number of minutes, hours or days, up to a week).
fn alert_pick(options: Value, current: String, on_pick: impl Fn(String) + 'static) -> gtk::Box {
    // What each item gives; the last is Custom….
    let values: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(arr(&options).iter().map(|o| s(o, "value").to_string()).collect()));
    let mut names: Vec<String> = arr(&options).iter().map(|o| s(o, "label").to_string()).collect();
    names.push("Custom…".into());
    let model = gtk::StringList::new(&names.iter().map(String::as_str).collect::<Vec<_>>());
    let drop = gtk::DropDown::builder().model(&model).css_classes(["field"]).build();
    drop.set_selected(values.borrow().iter().position(|v| *v == current).unwrap_or(0) as u32);
    let custom = gtk::Box::builder().spacing(8).visible(false).build();
    let n = gtk::SpinButton::with_range(1.0, 10080.0, 1.0);
    n.set_value(15.0);
    n.add_css_class("field");
    let unit = gtk::DropDown::from_strings(&["minutes", "hours", "days"]);
    unit.add_css_class("field");
    let set = uuob_kit_gtk::btn("Set", None, &["primary", "sm"]);
    set.set_valign(gtk::Align::Center);
    custom.append(&n);
    custom.append(&unit);
    custom.append(&gtk::Label::builder().label("before").css_classes(["secondary"]).build());
    custom.append(&set);
    let on_pick = Rc::new(on_pick);
    let quiet = Rc::new(Cell::new(false));
    // The sheet around it grows to show the custom line (a dialog keeps its first size otherwise).
    let grow = |w: &gtk::Box| {
        let w = w.clone();
        glib::idle_add_local_once(move || {
            if let Some(d) = w.ancestor(adw::Dialog::static_type()).and_then(|d| d.downcast::<adw::Dialog>().ok()) {
                if let Some(child) = d.child() {
                    let width = d.content_width().max(child.width());
                    let (_, natural, _, _) = child.measure(gtk::Orientation::Vertical, width);
                    d.set_content_height(natural.min(760));
                }
            }
        });
    };
    {
        let (custom, on_pick, values, quiet) = (custom.clone(), on_pick.clone(), values.clone(), quiet.clone());
        drop.connect_selected_notify(move |d| {
            if quiet.get() {
                return;
            }
            let i = d.selected() as usize;
            let vals = values.borrow();
            custom.set_visible(i >= vals.len());
            grow(&custom);
            if i < vals.len() {
                on_pick(vals[i].clone());
            }
        });
    }
    {
        let (on_pick, custom, drop, model, values, quiet) = (on_pick.clone(), custom.clone(), drop.clone(), model.clone(), values.clone(), quiet.clone());
        set.connect_clicked(move |_| {
            let per = [1.0, 60.0, 1440.0][unit.selected() as usize % 3];
            let m = (n.value() * per).round() as i64;
            if m <= 0 {
                return;
            }
            let m = m.min(calendar_core::upcoming::MAX_BEFORE);
            let v = m.to_string();
            // The chosen time shows in the list, as its own item. (Adding it moves the
            // selection at once, so the list doesn't answer until it's settled.)
            quiet.set(true);
            let at = {
                let mut vals = values.borrow_mut();
                match vals.iter().position(|x| *x == v) {
                    Some(i) => i,
                    None => {
                        let i = vals.len();
                        vals.push(v.clone());
                        model.splice(i as u32, 0, &[calendar_core::upcoming::before_label(Some(m)).as_str()]);
                        i
                    }
                }
            };
            drop.set_selected(at as u32);
            quiet.set(false);
            custom.set_visible(false);
            grow(&custom);
            on_pick(v);
        });
    }
    let col = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(6).hexpand(true).build();
    col.append(&drop);
    col.append(&custom);
    col
}

/// A date picked from a calendar.
fn date_button(date: &str, on_pick: impl Fn(String) + 'static) -> gtk::MenuButton {
    let shown = gtk::Label::builder().label(dates::friendly(date)).xalign(0.0).hexpand(true).build();
    let btn = gtk::MenuButton::builder().child(&shown).always_show_arrow(true).hexpand(true).css_classes(["field"]).build();
    let cal = gtk::Calendar::new();
    if let Some(d) = dates::parse(date) {
        if let Ok(dt) = glib::DateTime::from_local(d.year() as i32, d.month() as i32, d.day() as i32, 12, 0, 0.0) {
            cal.select_day(&dt);
        }
    }
    let pop = gtk::Popover::builder().child(&cal).build();
    btn.set_popover(Some(&pop));
    cal.connect_day_selected(move |c| {
        let d = c.date();
        let iso = format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day_of_month());
        shown.set_label(&dates::friendly(&iso));
        pop.popdown();
        on_pick(iso);
    });
    btn
}

/// A dialog with a header bar and scrolling content.
fn dialog(title: &str, width: i32, content: &gtk::Box) -> (adw::Dialog, adw::HeaderBar) {
    let d = adw::Dialog::builder().title(title).content_width(width).build();
    let view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    view.add_top_bar(&header);
    let scroll = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).propagate_natural_height(true).max_content_height(720).child(content).build();
    view.set_content(Some(&scroll));
    d.set_child(Some(&view));
    (d, header)
}

fn form() -> gtk::Box {
    gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(12).margin_start(20).margin_end(20).margin_bottom(20).build()
}

fn icon_button(icon: &str, text: &str) -> gtk::Button {
    let b = uuob_kit_gtk::btn(text, Some(icon), &["ghost"]);
    b.set_halign(gtk::Align::Start);
    b
}

impl Calendar {
    fn main_id(&self) -> String {
        arr(&self.data["containers"]).iter().find(|c| c["main"] == json!(true)).map(|c| s(c, "id").to_string()).unwrap_or_else(|| "calendar".into())
    }

    fn state(&self) -> Value {
        let mut st = self.data.clone();
        st["view"] = json!(self.view);
        st["cursor"] = json!(self.cursor);
        st["narrow"] = json!(self.narrow);
        st["sunday_first"] = json!(uuob_kit_gtk::sunday_first(&self.client));
        st["hour12"] = json!(uuob_kit_gtk::hour12(&self.client));
        st["now"] = json!(zones::now_ms());
        st["tasks"] = json!(self.tasks);
        if let Some(h) = &self.hidden {
            st["hidden"] = json!(h);
        }
        if let Some(t) = self.show_tasks {
            st["show_tasks"] = json!(t);
        }
        if let Some(v) = &self.views {
            st["views"] = v.clone();
        }
        st
    }

    /// The first and last day on screen.
    fn range(&self) -> (String, String) {
        let d = strs(&self.screen["days"]);
        (d.first().cloned().unwrap_or_default(), d.last().cloned().unwrap_or_default())
    }

    fn emails(&self) -> Vec<String> {
        strs(&self.screen["emails"])
    }

    /// What is coming up is written 1.5 s after the last change, once every
    /// calendar and the preferences have been read.
    fn schedule_upcoming(&mut self, sender: &ComponentSender<Self>) {
        if !self.events_known || !self.prefs_known {
            return;
        }
        self.upcoming_gen += 1;
        let generation = self.upcoming_gen;
        let tx = sender.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(1500), move || tx.input(Msg::Upcoming(generation)));
    }

    fn render(&mut self, w: &Widgets, sender: &ComponentSender<Self>) {
        let screen = app::screen(&self.state());
        self.view = s(&screen, "view").to_string();
        self.screen = screen.clone();
        clear(&w.cals);
        for c in arr(&screen["calendars"]) {
            let row = gtk::Box::builder().spacing(12).css_classes(["cal-row"]).build();
            let check = gtk::CheckButton::builder().active(c["hidden"] != json!(true)).css_classes([format!("uuob-check-{}", s(&c, "color"))]).build();
            let id = s(&c, "id").to_string();
            let tx = sender.clone();
            check.connect_toggled(move |_| tx.input(Msg::ToggleCal(id.clone())));
            row.append(&check);
            row.append(&gtk::Label::builder().label(s(&c, "name")).hexpand(true).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).css_classes(["cal-name"]).build());
            if c["subscribed"] == json!(true) {
                let i = uuob_kit_gtk::icon("link", 15, 1.75, "text3");
                i.set_tooltip_text(Some("Subscribed to a link"));
                row.append(&i);
            }
            if c["shared"] == json!(true) {
                let i = uuob_kit_gtk::icon("people", 15, 1.75, "text3");
                i.set_tooltip_text(Some(s(&c, "note")));
                row.append(&i);
            }
            let edit = uuob_kit_gtk::icon_btn("edit", 15, if c["from"].is_object() { "About this calendar" } else { "Edit" });
            edit.add_css_class("sm");
            let (tx, cc) = (sender.clone(), c.clone());
            edit.connect_clicked(move |_| tx.input(Msg::CalSheet(Some(cc.clone()))));
            row.append(&edit);
            w.cals.append(&gtk::ListBoxRow::builder().child(&row).activatable(false).build());
        }
        w.tasks_check.set_active(screen["showTasks"] == json!(true));
        w.title.set_markup(&format!("<b>{}</b> <span weight=\"normal\" alpha=\"62%\">{}</span>", s(&screen["title"], "month"), screen["title"]["year"]));
        clear(&w.switch);
        for v in arr(&screen["views"]) {
            let b = gtk::ToggleButton::builder().label(s(&v, "name")).active(s(&v, "id") == self.view).css_classes(["flat"]).build();
            let id = s(&v, "id").to_string();
            let tx = sender.clone();
            b.connect_clicked(move |_| tx.input(Msg::Show(id.clone())));
            w.switch.append(&b);
        }
        if self.view == "month" {
            self.render_month(&screen, w, sender);
            w.body.set_visible_child_name("month");
        } else {
            self.render_week(&screen, w, sender);
            w.body.set_visible_child_name("week");
        }
    }

    fn render_week(&self, screen: &Value, w: &Widgets, sender: &ComponentSender<Self>) {
        let hour12 = uuob_kit_gtk::hour12(&self.client);
        let cols = arr(&screen["columns"]);
        clear(&w.heads);
        clear(&w.allday);
        clear(&w.columns);
        w.heads.append(&gtk::Box::builder().width_request(56).build());
        w.allday.append(&gtk::Label::builder().label("all-day").width_request(56).valign(gtk::Align::Start).css_classes(["caption", "dim-label", "gutter"]).build());
        // The days share the width equally, however long a title is (as the grid's columns do).
        let heads_in = gtk::Box::builder().homogeneous(true).hexpand(true).build();
        let allday_in = gtk::Box::builder().homogeneous(true).hexpand(true).build();
        w.heads.append(&heads_in);
        w.allday.append(&allday_in);
        let day_view = s(screen, "dayView").to_string();
        for col in &cols {
            let date = s(col, "date").to_string();
            let head = gtk::Button::builder().css_classes(["flat", "head"]).hexpand(true).build();
            let hb = gtk::Box::builder().spacing(6).halign(gtk::Align::Center).build();
            hb.append(&gtk::Label::builder().label(s(col, "weekday").to_uppercase()).valign(gtk::Align::Center).css_classes(["wd"]).build());
            let n = gtk::Label::builder().label(col["day"].to_string()).css_classes(["dn"]).build();
            if col["today"] == json!(true) {
                head.add_css_class("today");
            }
            hb.append(&n);
            head.set_child(Some(&hb));
            if cols.len() > 1 && day_view != self.view {
                let (tx, d) = (sender.clone(), date.clone());
                head.connect_clicked(move |_| tx.input(Msg::Day(d.clone())));
            }
            heads_in.append(&head);
            let cell = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(2).hexpand(true).css_classes(["ad-cell"]).build();
            for p in arr(&col["allDay"]) {
                let b = gtk::Button::builder().css_classes(["ad-pill", &format!("tint-{}", s(&p, "color"))]).tooltip_text(s(&p, "title")).build();
                let line = gtk::Box::builder().spacing(4).build();
                if p["repeats"] == json!(true) {
                    line.append(&uuob_kit_gtk::icon("repeat", 11, 2.2, &tint_hex(s(&p, "color"))));
                }
                line.append(&gtk::Label::builder().label(s(&p, "title")).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).build());
                b.set_child(Some(&line));
                if p["continues"] == json!(true) {
                    b.add_css_class("cont");
                }
                if p["goesOn"] == json!(true) {
                    b.add_css_class("on");
                }
                answer_class(&b, &p["answer"]);
                let (tx, key) = (sender.clone(), s(&p, "key").to_string());
                b.connect_clicked(move |_| tx.input(Msg::Open(key.clone())));
                cell.append(&b);
            }
            for t in arr(&col["tasksAllDay"]) {
                cell.append(&task_line(s(&t, "title"), &["ad-pill", "task"]));
            }
            allday_in.append(&cell);
            w.columns.append(&day_column(col.clone(), screen["nowMinute"].as_i64().unwrap_or(0), hour12, sender));
        }
        w.hours.set_draw_func(move |area, cr, wd, _| {
            let fg = area.color();
            cr.set_source_rgba(fg.red() as f64, fg.green() as f64, fg.blue() as f64, 0.38);
            let layout = area.create_pango_layout(None);
            let mut font = gtk::pango::FontDescription::new();
            font.set_absolute_size(11.0 * gtk::pango::SCALE as f64);
            layout.set_font_description(Some(&font));
            for h in 1..24 {
                layout.set_text(&dates::friendly_time(&format!("{h:02}:00"), hour12));
                let (tw, th) = layout.pixel_size();
                cr.move_to(wd as f64 - tw as f64 - 8.0, h as f64 * HOUR - th as f64 / 2.0);
                pangocairo::functions::show_layout(cr, &layout);
            }
        });
        w.hours.queue_draw();
    }

    fn render_month(&self, screen: &Value, w: &Widgets, sender: &ComponentSender<Self>) {
        clear(&w.month);
        for (i, h) in arr(&screen["monthHeads"]).iter().enumerate() {
            w.month.attach(&gtk::Label::builder().label(h.as_str().unwrap_or("").to_uppercase()).xalign(if self.narrow { 0.5 } else { 1.0 }).css_classes(["mh"]).build(), i as i32, 0, 1, 1);
        }
        for (i, c) in arr(&screen["cells"]).iter().enumerate() {
            let cell = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(2).hexpand(true).vexpand(true).css_classes(["cell"]).build();
            if c["other"] == json!(true) {
                cell.add_css_class("other");
            }
            let num = gtk::Button::builder().label(c["day"].to_string()).halign(if self.narrow { gtk::Align::Center } else { gtk::Align::End }).css_classes(["flat", "num"]).build();
            if c["today"] == json!(true) {
                num.add_css_class("today");
            }
            let date = s(c, "date").to_string();
            let (tx, d) = (sender.clone(), date.clone());
            num.connect_clicked(move |_| tx.input(Msg::Day(d.clone())));
            cell.append(&num);
            for it in arr(&c["items"]) {
                let b = gtk::Button::builder().css_classes(["flat", "mev"]).build();
                let line = gtk::Box::builder().spacing(4).build();
                if it["solid"] == json!(true) {
                    b.add_css_class(&format!("tint-{}", s(&it, "color")));
                    b.add_css_class("solid");
                } else {
                    line.append(&gtk::Box::builder().valign(gtk::Align::Center).css_classes(["dot", &format!("uuob-c-{}", s(&it, "color"))]).build());
                }
                // Phones show only the dots (and the bars of all-day events), as on the web.
                let text = if self.narrow { String::new() } else if s(&it, "time").is_empty() { s(&it, "title").to_string() } else { format!("{} {}", s(&it, "time"), s(&it, "title")) };
                line.append(&gtk::Label::builder().label(text).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).build());
                b.set_child(Some(&line));
                answer_class(&b, &it["answer"]);
                let (tx, key) = (sender.clone(), s(&it, "key").to_string());
                b.connect_clicked(move |_| tx.input(Msg::Open(key.clone())));
                cell.append(&b);
            }
            if c["more"].as_u64().unwrap_or(0) > 0 {
                let more = if self.narrow { format!("+{}", c["more"]) } else { format!("+{} more", c["more"]) };
                cell.append(&gtk::Label::builder().label(more).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).css_classes(["more"]).build());
            }
            for t in arr(&c["tasks"]) {
                cell.append(&task_line(s(&t, "title"), &["mev", "task"]));
            }
            // A click on the day itself makes an event there.
            let click = gtk::GestureClick::new();
            let tx = sender.clone();
            click.connect_released(move |_, _, _, _| tx.input(Msg::NewEvent(date.clone(), None, None)));
            cell.add_controller(click);
            w.month.attach(&cell, (i % 7) as i32, (i / 7) as i32 + 1, 1, 1);
        }
    }

    /// The card: the event's lines, Join, people, notes, your answer.
    fn show_card(&self, key: &str, root: &adw::ApplicationWindow, sender: &ComponentSender<Self>) {
        let (from, to) = self.range();
        let c = app::card(&self.state(), key, &from, &to);
        if c.is_null() {
            return;
        }
        let col = form();
        col.set_spacing(8);
        col.append(&label(s(&c, "title"), &["card-title"]));
        col.append(&label(s(&c, "when"), &[]));
        if let Some(r) = c["repeat"].as_str() {
            col.append(&label(r, &["caption", "dim-label"]));
        }
        let cal = gtk::Box::builder().spacing(6).build();
        cal.append(&gtk::Box::builder().valign(gtk::Align::Center).css_classes(["dot", &format!("uuob-c-{}", s(&c, "color"))]).build());
        cal.append(&label(s(&c, "calendar"), &["caption", "dim-label"]));
        col.append(&cal);
        if c["meeting"].is_object() {
            let m = &c["meeting"];
            let b = gtk::Button::builder().css_classes(["btn", "primary", "lg"]).build();
            let bx = gtk::Box::builder().spacing(8).halign(gtk::Align::Center).build();
            bx.append(&uuob_kit_gtk::icon("video", 18, 2.0, "#ffffff"));
            bx.append(&gtk::Label::new(Some(&format!("Join {}", s(m, "name")))));
            bx.append(&gtk::Label::builder().label(s(m, "host")).css_classes(["join-host"]).build());
            b.set_child(Some(&bx));
            let url = s(m, "url").to_string();
            b.connect_clicked(move |_| open_uri(&url));
            col.append(&b);
        }
        if !s(&c, "place").is_empty() {
            let row = gtk::Box::builder().spacing(8).build();
            row.append(&uuob_kit_gtk::icon("place", 17, 1.75, uuob_kit_gtk::text2()));
            row.append(&label(s(&c, "place"), &[]));
            col.append(&row);
        }
        if let Some(u) = c["url"].as_str() {
            let l = label("", &[]);
            l.set_markup(&format!("<a href=\"{}\">{}</a>", glib::markup_escape_text(u), glib::markup_escape_text(s(&c, "url_shown"))));
            col.append(&l);
        }
        if c["timed"] == json!(true) {
            let row = gtk::Box::builder().spacing(8).build();
            let bell = uuob_kit_gtk::icon("bell", 17, 1.75, uuob_kit_gtk::text2());
            bell.set_valign(gtk::Align::Start);
            bell.set_margin_top(14);
            row.append(&bell);
            let (tx, ak) = (sender.clone(), s(&c, "answerKey").to_string());
            row.append(&alert_pick(c["alertOptions"].clone(), s(&c, "alert").to_string(), move |v| tx.input(Msg::OwnAlert(ak.clone(), v))));
            col.append(&row);
        }
        let people = arr(&c["people"]);
        if !people.is_empty() {
            let ex = gtk::Expander::builder().label(s(&c, "peopleSummary")).build();
            let list = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(4).margin_top(6).build();
            for p in people {
                let row = gtk::Box::builder().spacing(8).build();
                let mark = match p["answer"].as_str() {
                    Some("yes") => "✓",
                    Some("maybe") => "?",
                    Some("no") => "✕",
                    _ => "",
                };
                row.append(&gtk::Label::builder().label(mark).css_classes(["people-st"]).build());
                let name = p["name"].as_str().filter(|n| !n.is_empty()).unwrap_or(s(&p, "email"));
                row.append(&gtk::Label::builder().label(name).xalign(0.0).hexpand(true).ellipsize(gtk::pango::EllipsizeMode::End).build());
                for (k, t) in [("organizer", "organizer"), ("you", "you")] {
                    if p[k] == json!(true) {
                        row.append(&gtk::Label::builder().label(t).css_classes(["caption", "dim-label"]).build());
                    }
                }
                list.append(&row);
            }
            ex.set_child(Some(&list));
            col.append(&ex);
        }
        if c["hasNotes"] == json!(true) {
            col.append(&linked(&c["notes"], &[]));
        }
        if c["hasJoining"] == json!(true) {
            let ex = gtk::Expander::builder().label("Joining details").build();
            ex.set_child(Some(&linked(&c["joining"], &["caption"])));
            col.append(&ex);
        }
        if c["asksAnswer"] == json!(true) {
            let current = c["answer"].as_str().unwrap_or("").to_string();
            let (tx, ak) = (sender.clone(), s(&c, "answerKey").to_string());
            let opts = ["yes", "maybe", "no"];
            let row = uuob_kit_gtk::segmented(&["Going", "Maybe", "No"], opts.iter().position(|o| *o == current).unwrap_or(9), move |i| tx.input(Msg::Answer(ak.clone(), opts[i].into())));
            row.set_halign(gtk::Align::Center);
            row.set_margin_top(8);
            col.append(&row);
            col.append(&gtk::Label::builder().label("Your answer stays on your devices.").css_classes(["caption", "dim-label"]).build());
        }
        let (d, header) = dialog("", 420, &col);
        if c["canEdit"] == json!(true) {
            let edit = uuob_kit_gtk::icon_btn("edit", 18, "Edit");
            let (tx, key, dd) = (sender.clone(), key.to_string(), d.clone());
            edit.connect_clicked(move |_| {
                dd.close();
                tx.input(Msg::Edit(key.clone()));
            });
            header.pack_end(&edit);
        }
        d.present(Some(root));
    }

    /// The event sheet: a new event, or one being changed.
    fn show_sheet(&self, ed: Value, root: &adw::ApplicationWindow, sender: &ComponentSender<Self>) {
        let hour12 = uuob_kit_gtk::hour12(&self.client);
        let ed = Rc::new(RefCell::new(ed));
        let col = form();
        let title = gtk::Entry::builder().placeholder_text("New event").text(s(&ed.borrow(), "title")).css_classes(["field", "field-title"]).build();
        col.append(&title);
        let all_day = gtk::Switch::builder().active(ed.borrow()["all_day"] == json!(true)).valign(gtk::Align::Center).build();
        let ad_row = gtk::Box::builder().spacing(10).build();
        ad_row.append(&gtk::Label::builder().label("All-day").hexpand(true).xalign(0.0).build());
        ad_row.append(&all_day);
        col.append(&ad_row);
        // Starts and ends (dates, and times unless all-day).
        let times_box = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(8).build();
        col.append(&times_box);
        let build_times: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let bt: Rc<dyn Fn()> = {
            let (ed, times_box, me) = (ed.clone(), times_box.clone(), build_times.clone());
            Rc::new(move || {
                clear(&times_box);
                let e = ed.borrow().clone();
                let all = e["all_day"] == json!(true);
                for (lab, dk, tk) in [("Starts", "date", "start"), ("Ends", "end_date", "end")] {
                    let row = gtk::Box::builder().spacing(10).homogeneous(true).build();
                    let st = stack_box(lab);
                    row.append(&st);
                    let (ed2, me2) = (ed.clone(), me.clone());
                    let starts = dk == "date";
                    st.append(&date_button(s(&e, dk), move |d| {
                        let next = if starts {
                            app::move_start(&ed2.borrow(), &d)
                        } else {
                            let mut x = ed2.borrow().clone();
                            x["end_date"] = json!(d);
                            x
                        };
                        *ed2.borrow_mut() = next;
                        // A new start moves the end too.
                        if starts {
                            if let Some(f) = me2.borrow().clone() {
                                glib::idle_add_local_once(move || f());
                            }
                        }
                    }));
                    if !all {
                        let (ed3, tk) = (ed.clone(), tk.to_string());
                        let tf = uuob_kit_gtk::time_field(Some(s(&e, &tk).to_string()), hour12, false, "", move |t| {
                            if let Some(t) = t {
                                ed3.borrow_mut()[tk.as_str()] = json!(t);
                            }
                        });
                        tf.set_valign(gtk::Align::End);
                        row.append(&tf);
                    }
                    times_box.append(&row);
                }
            })
        };
        *build_times.borrow_mut() = Some(bt.clone());
        bt();
        {
            let (ed, bt) = (ed.clone(), bt.clone());
            all_day.connect_active_notify(move |sw| {
                ed.borrow_mut()["all_day"] = json!(sw.is_active());
                bt();
            });
        }
        // Repeat.
        let rep_box = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(8).build();
        col.append(&rep_box);
        let build_repeat: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let br: Rc<dyn Fn()> = {
            let (ed, rep_box, me) = (ed.clone(), rep_box.clone(), build_repeat.clone());
            Rc::new(move || {
                clear(&rep_box);
                let again = {
                    let me = me.clone();
                    move || {
                        if let Some(f) = me.borrow().clone() {
                            glib::idle_add_local_once(move || f());
                        }
                    }
                };
                let r = ed.borrow()["repeat"].clone();
                let freq = s(&r, "freq").to_string();
                let mut freqs: Vec<(String, String)> = [("NONE", "Never"), ("DAILY", "Every day"), ("WEEKLY", "Every week"), ("MONTHLY", "Every month"), ("YEARLY", "Every year")].iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
                if r["raw"].is_string() {
                    freqs.push(("CUSTOM".into(), s(&r, "label").to_string()));
                }
                let row = stack_box("Repeat");
                let drop = gtk::DropDown::from_strings(&freqs.iter().map(|f| f.1.as_str()).collect::<Vec<_>>());
                drop.add_css_class("field");
                drop.set_hexpand(true);
                drop.set_selected(freqs.iter().position(|f| f.0 == freq).unwrap_or(0) as u32);
                {
                    let (ed, again, codes) = (ed.clone(), again.clone(), freqs.iter().map(|f| f.0.clone()).collect::<Vec<_>>());
                    drop.connect_selected_notify(move |d| {
                        ed.borrow_mut()["repeat"]["freq"] = json!(codes[d.selected() as usize]);
                        again();
                    });
                }
                row.append(&drop);
                rep_box.append(&row);
                if freq == "WEEKLY" {
                    let days = gtk::Box::builder().spacing(6).halign(gtk::Align::Center).build();
                    for (code, letter) in [("MO", "M"), ("TU", "T"), ("WE", "W"), ("TH", "T"), ("FR", "F"), ("SA", "S"), ("SU", "S")] {
                        let on = strs(&r["days"]).iter().any(|d| d == code);
                        let b = gtk::ToggleButton::builder().label(letter).active(on).css_classes(["wday"]).tooltip_text(code).build();
                        let ed = ed.clone();
                        b.connect_toggled(move |b| {
                            let mut list = strs(&ed.borrow()["repeat"]["days"]);
                            if b.is_active() {
                                if !list.iter().any(|d| d == code) {
                                    list.push(code.into());
                                }
                            } else if list.len() > 1 {
                                list.retain(|d| d != code);
                            } else {
                                b.set_active(true); // at least one day
                                return;
                            }
                            ed.borrow_mut()["repeat"]["days"] = json!(list);
                        });
                        days.append(&b);
                    }
                    rep_box.append(&days);
                }
                if freq != "NONE" && freq != "CUSTOM" {
                    let unit = match freq.as_str() {
                        "DAILY" => "days",
                        "WEEKLY" => "weeks",
                        "MONTHLY" => "months",
                        _ => "years",
                    };
                    let every_st = stack_box("Every");
                    let every = gtk::Box::builder().spacing(10).build();
                    every_st.append(&every);
                    let n = gtk::SpinButton::with_range(1.0, 99.0, 1.0);
                    n.add_css_class("field");
                    n.set_value(r["interval"].as_f64().unwrap_or(1.0));
                    let ed2 = ed.clone();
                    n.connect_value_changed(move |n| ed2.borrow_mut()["repeat"]["interval"] = json!(n.value() as i64));
                    every.append(&n);
                    every.append(&gtk::Label::builder().label(unit).css_classes(["unit"]).build());
                    let until = stack_box("Until");
                    let ends = ["never", "until", "count"];
                    let ends_drop = gtk::DropDown::from_strings(&["Forever", "A date", "A number of times"]);
                    ends_drop.add_css_class("field");
                    ends_drop.set_selected(ends.iter().position(|e| *e == s(&r, "ends")).unwrap_or(0) as u32);
                    {
                        let (ed, again) = (ed.clone(), again.clone());
                        ends_drop.connect_selected_notify(move |d| {
                            ed.borrow_mut()["repeat"]["ends"] = json!(ends[d.selected() as usize]);
                            again();
                        });
                    }
                    until.append(&ends_drop);
                    let two = gtk::Box::builder().spacing(10).homogeneous(true).build();
                    two.append(&every_st);
                    two.append(&until);
                    rep_box.append(&two);
                    match s(&r, "ends") {
                        "until" => {
                            let ed = ed.clone();
                            rep_box.append(&date_button(s(&r, "until"), move |d| ed.borrow_mut()["repeat"]["until"] = json!(d)));
                        }
                        "count" => {
                            let c = gtk::SpinButton::with_range(1.0, 999.0, 1.0);
                            c.add_css_class("field");
                            c.set_value(r["count"].as_f64().unwrap_or(10.0));
                            let ed = ed.clone();
                            c.connect_value_changed(move |c| ed.borrow_mut()["repeat"]["count"] = json!(c.value() as i64));
                            let line = gtk::Box::builder().spacing(10).build();
                            line.append(&c);
                            line.append(&gtk::Label::builder().label("times").css_classes(["unit"]).build());
                            rep_box.append(&line);
                        }
                        _ => {}
                    }
                }
            })
        };
        *build_repeat.borrow_mut() = Some(br.clone());
        br();
        // Alert, calendar, place, notes.
        if ed.borrow()["all_day"] != json!(true) {
            let row = stack_box("Alert");
            let before = self.screen["before"].as_i64();
            let current = s(&ed.borrow(), "alert").to_string();
            let ed2 = ed.clone();
            row.append(&alert_pick(app::alert_options(&current, before, true), current, move |v| ed2.borrow_mut()["alert"] = json!(v)));
            col.append(&row);
        }
        let cals: Vec<Value> = arr(&self.screen["calendars"]).into_iter().filter(|c| c["canEdit"] == json!(true) || c["id"] == ed.borrow()["calendar"]).collect();
        let row = stack_box("Calendar");
        let cal_drop = gtk::DropDown::from_strings(&cals.iter().map(|c| s(c, "name")).collect::<Vec<_>>());
        cal_drop.add_css_class("field");
        cal_drop.set_hexpand(true);
        cal_drop.set_selected(cals.iter().position(|c| c["id"] == ed.borrow()["calendar"]).unwrap_or(0) as u32);
        {
            let (ed, cals) = (ed.clone(), cals.clone());
            cal_drop.connect_selected_notify(move |d| {
                if let Some(c) = cals.get(d.selected() as usize) {
                    ed.borrow_mut()["calendar"] = c["id"].clone();
                }
            });
        }
        row.append(&cal_drop);
        col.append(&row);
        let place = gtk::Entry::builder().placeholder_text("Location").text(s(&ed.borrow(), "location")).css_classes(["field"]).build();
        col.append(&place);
        let notes = gtk::TextView::builder().wrap_mode(gtk::WrapMode::WordChar).top_margin(4).bottom_margin(4).left_margin(10).right_margin(10).height_request(90).css_classes(["field"]).build();
        notes.buffer().set_text(s(&ed.borrow(), "notes"));
        col.append(&notes);
        let is_new = ed.borrow()["id"].as_str().is_none_or(str::is_empty);
        let (d, _) = dialog(if is_new { "New event" } else { "Edit event" }, 460, &col);
        let actions = gtk::Box::builder().spacing(10).homogeneous(true).margin_top(6).build();
        let left = uuob_kit_gtk::btn(if is_new { "Cancel" } else { "Delete" }, None, &["lg"]);
        if !is_new {
            left.add_css_class("danger");
        }
        let save = uuob_kit_gtk::btn(if is_new { "Add" } else { "Save" }, None, &["lg", "primary"]);
        let collect = {
            let (ed, title, place, notes) = (ed.clone(), title.clone(), place.clone(), notes.clone());
            Rc::new(move || {
                let mut e = ed.borrow().clone();
                e["title"] = json!(title.text().to_string());
                e["location"] = json!(place.text().to_string());
                let b = notes.buffer();
                e["notes"] = json!(b.text(&b.start_iter(), &b.end_iter(), false).to_string());
                e
            })
        };
        {
            let (tx, dd, collect) = (sender.clone(), d.clone(), collect.clone());
            left.connect_clicked(move |_| {
                dd.close();
                if !is_new {
                    tx.input(Msg::Delete(collect(), None));
                }
            });
        }
        {
            let (tx, dd, collect2) = (sender.clone(), d.clone(), collect.clone());
            save.connect_clicked(move |_| {
                dd.close();
                tx.input(Msg::Save(collect2(), None));
            });
            let (tx, dd) = (sender.clone(), d.clone());
            title.connect_activate(move |_| {
                dd.close();
                tx.input(Msg::Save(collect(), None));
            });
        }
        actions.append(&left);
        actions.append(&save);
        col.append(&actions);
        d.present(Some(root));
    }

    /// "Only this one, or all of them?" for a repeating event.
    fn ask_scope(&self, ed: Value, delete: bool, root: &adw::ApplicationWindow, sender: &ComponentSender<Self>) {
        let verb = if delete { "Delete" } else { "Change" };
        let day = s(&ed, "occurrence").to_string();
        let tx = sender.clone();
        let (this, all): (&[&str], &[&str]) = if delete { (&["danger"], &["danger"]) } else { (&[], &["primary"]) };
        let a = uuob_kit_gtk::confirm_sheet(&format!("{verb} a repeating event"), &format!("{verb} only this one, on {}, or every time it repeats?", dates::friendly(&day)), &[("Only this one", this, "this"), ("All of them", all, "all")], true, move |r| {
            let scope = Some(r.to_string());
            tx.input(if delete { Msg::Delete(ed.clone(), scope) } else { Msg::Save(ed.clone(), scope) });
        });
        a.present(Some(root));
    }

    /// New calendar / edit one / about one shared with you.
    fn show_cal_sheet(&self, c: Option<Value>, root: &adw::ApplicationWindow, sender: &ComponentSender<Self>) {
        let n = arr(&self.screen["calendars"]).len();
        let c = c.unwrap_or_else(|| json!({ "name": "", "color": app::COLORS[(n + 1) % app::COLORS.len()] }));
        let id = c["id"].as_str().map(String::from);
        let shared = c["from"].is_object();
        let col = form();
        if shared {
            let can = if c["canWrite"] == json!(true) { "You can both change it." } else { "You can view it, but not change it." };
            col.append(&label(&format!("{} shares this calendar with you. {can}", s(&c["from"], "name")), &["dim-label"]));
        }
        let name = gtk::Entry::builder().placeholder_text("Name, e.g. Work").text(s(&c, "name")).visible(!shared).css_classes(["field"]).build();
        col.append(&name);
        let color = Rc::new(RefCell::new(s(&c, "color").to_string()));
        let sw = gtk::Box::builder().spacing(12).halign(gtk::Align::Center).visible(!shared).build();
        let buttons: Rc<RefCell<Vec<(&str, gtk::Button)>>> = Rc::new(RefCell::new(vec![]));
        for k in uuob_kit_gtk::COLORS {
            let b = gtk::Button::builder().tooltip_text(k).css_classes(["swatch", &format!("uuob-c-{k}")]).build();
            if *color.borrow() == k {
                b.add_css_class("on");
            }
            let (color, all) = (color.clone(), buttons.clone());
            b.connect_clicked(move |_| {
                *color.borrow_mut() = k.to_string();
                for (kk, bb) in all.borrow().iter() {
                    if *kk == k { bb.add_css_class("on") } else { bb.remove_css_class("on") }
                }
            });
            sw.append(&b);
            buttons.borrow_mut().push((k, b));
        }
        col.append(&sw);
        let title = if shared { s(&c, "name").to_string() } else if id.is_some() { "Edit calendar".into() } else { "New calendar".into() };
        let (d, _) = dialog(&title, 400, &col);
        if c["subscribed"] == json!(true) {
            let subs = arr(&self.data["subs"]);
            let sub = subs.iter().find(|x| x["id"] == c["id"]);
            let when = sub.map(|x| format!(", updated {}", app::ago(x["refreshed"].as_i64().unwrap_or(0), zones::now_ms()))).unwrap_or_default();
            col.append(&label(&format!("Follows a calendar link{when}. Its events change only at the link."), &["caption", "dim-label"]));
            if sub.is_some() {
                let b = icon_button("repeat", "Update now");
                let (tx, id2, dd) = (sender.clone(), s(&c, "id").to_string(), d.clone());
                b.connect_clicked(move |_| {
                    dd.close();
                    tx.input(Msg::RefreshNow(id2.clone()));
                });
                col.append(&b);
            }
        }
        if let Some(id) = &id {
            let b = icon_button("download", "Save as .ics file");
            let (tx, id2) = (sender.clone(), id.clone());
            b.connect_clicked(move |_| tx.input(Msg::Export(id2.clone())));
            col.append(&b);
        }
        if c["main"] == json!(true) {
            col.append(&label("Your first calendar stays private. To share events with someone, add a calendar and share that.", &["caption", "dim-label"]));
        } else if let (Some(id), false) = (&id, shared) {
            let with = arr(&c["sharedWith"]);
            let b = icon_button("share", if with.is_empty() { "Share calendar…" } else { s(&c, "note") });
            let (tx, id2, dd) = (sender.clone(), id.clone(), d.clone());
            b.connect_clicked(move |_| {
                dd.close();
                tx.input(Msg::Share(id2.clone()));
            });
            col.append(&b);
        }
        let actions = gtk::Box::builder().spacing(10).homogeneous(true).margin_top(6).build();
        let left_label = if shared { "Leave calendar" } else if id.is_some() && c["main"] != json!(true) { "Delete" } else { "Cancel" };
        let deletes = left_label != "Cancel";
        let left = uuob_kit_gtk::btn(left_label, None, &["lg"]);
        if deletes {
            left.add_css_class("danger");
        }
        {
            let (tx, dd, id, sub) = (sender.clone(), d.clone(), id.clone(), c["subscribed"] == json!(true));
            left.connect_clicked(move |_| {
                dd.close();
                if let (true, Some(id)) = (deletes, &id) {
                    tx.input(Msg::DeleteCal(id.clone(), sub));
                }
            });
        }
        let save = uuob_kit_gtk::btn(if shared { "Done" } else { "Save" }, None, &["lg", "primary"]);
        {
            let (tx, dd, name, color, c) = (sender.clone(), d.clone(), name.clone(), color.clone(), c.clone());
            save.connect_clicked(move |_| {
                dd.close();
                if !shared {
                    let mut out = c.clone();
                    out["name"] = json!(name.text().to_string());
                    out["color"] = json!(*color.borrow());
                    tx.input(Msg::SaveCal(out));
                }
            });
        }
        actions.append(&left);
        actions.append(&save);
        col.append(&actions);
        // Enter saves, as the web form did.
        {
            let save = save.clone();
            name.connect_activate(move |_| save.emit_clicked());
        }
        d.present(Some(root));
    }

    fn show_subscribe(&self, root: &adw::ApplicationWindow, sender: &ComponentSender<Self>) {
        let col = form();
        col.append(&label("A link to a calendar (.ics or webcal://), like your Google calendar's secret address or a holiday calendar.", &["dim-label"]));
        let url = gtk::Entry::builder().placeholder_text("https://…/calendar.ics").css_classes(["field"]).build();
        let name = gtk::Entry::builder().placeholder_text("Name (optional)").css_classes(["field"]).build();
        col.append(&url);
        col.append(&name);
        let go = uuob_kit_gtk::btn("Subscribe", None, &["lg", "primary"]);
        col.append(&go);
        // Enter subscribes, as the web form did.
        for e in [&url, &name] {
            let go = go.clone();
            e.connect_activate(move |_| go.emit_clicked());
        }
        let (d, _) = dialog("Subscribe to a link", 420, &col);
        let (tx, dd) = (sender.clone(), d.clone());
        go.connect_clicked(move |_| {
            let u = url.text().to_string();
            if !u.trim().is_empty() {
                dd.close();
                tx.input(Msg::Subscribe(u, name.text().to_string()));
            }
        });
        d.present(Some(root));
    }

    fn show_import(&self, items: Vec<Value>, root: &adw::ApplicationWindow, sender: &ComponentSender<Self>) {
        let col = form();
        let cals: Vec<Value> = arr(&self.screen["calendars"]).into_iter().filter(|c| c["canEdit"] == json!(true)).collect();
        let targets: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(items.iter().map(|_| "new".to_string()).collect()));
        for (i, it) in items.iter().enumerate() {
            let row = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(4).build();
            row.append(&label(s(it, "name"), &["heading"]));
            if let Some(e) = it["error"].as_str() {
                row.append(&label(e, &["caption", "error"]));
            } else {
                let rep = it["repeating"].as_u64().unwrap_or(0);
                row.append(&label(&format!("{} events{}", it["events"], if rep > 0 { format!(", {rep} repeating") } else { String::new() }), &["caption", "dim-label"]));
                let mut names = vec!["A new calendar".to_string()];
                names.extend(cals.iter().map(|c| format!("Into {}", s(c, "name"))));
                let drop = gtk::DropDown::from_strings(&names.iter().map(String::as_str).collect::<Vec<_>>());
                drop.add_css_class("field");
                let (targets, cals) = (targets.clone(), cals.clone());
                drop.connect_selected_notify(move |d| {
                    let k = d.selected() as usize;
                    targets.borrow_mut()[i] = if k == 0 { "new".into() } else { s(&cals[k - 1], "id").to_string() };
                });
                row.append(&drop);
            }
            col.append(&row);
        }
        let go = uuob_kit_gtk::btn("Import", None, &["lg", "primary"]);
        go.set_sensitive(items.iter().any(|i| i["error"].is_null()));
        col.append(&go);
        let (d, _) = dialog("Import", 420, &col);
        let (tx, dd) = (sender.clone(), d.clone());
        go.connect_clicked(move |_| {
            dd.close();
            let mut out = items.clone();
            for (i, it) in out.iter_mut().enumerate() {
                it["target"] = json!(targets.borrow()[i]);
            }
            tx.input(Msg::ImportRun(out));
        });
        d.present(Some(root));
    }

    fn show_settings(&self, root: &adw::ApplicationWindow, sender: &ComponentSender<Self>) {
        let key = if self.narrow { "narrow" } else { "wide" };
        let sw = self.screen["switch"][key].clone();
        let mut view_rows = vec![];
        let three = sw["three"] == json!(true);
        {
            let (tx, k) = (sender.clone(), key.to_string());
            view_rows.push(uuob_kit_gtk::choice_row("Third view", &["Off", "On"], three as usize, move |i| tx.input(Msg::SetSwitch(k.clone(), json!({ "three": i == 1 })))));
        }
        let slots: &[(&str, &str)] = if three { &[("small", "Smallest"), ("mid", "Middle"), ("big", "Biggest")] } else { &[("small", "Small"), ("big", "Big")] };
        for (slot, name) in slots {
            let drop = gtk::DropDown::from_strings(&app::VIEWS.iter().map(|v| v.1).collect::<Vec<_>>());
            drop.add_css_class("field");
            drop.set_width_request(170);
            drop.set_selected(app::VIEWS.iter().position(|v| v.0 == s(&sw, slot)).unwrap_or(0) as u32);
            let (tx, k, slot) = (sender.clone(), key.to_string(), slot.to_string());
            let others: Vec<String> = slots.iter().filter(|(o, _)| *o != slot).map(|(o, _)| s(&sw, o).to_string()).collect();
            drop.connect_selected_notify(move |d| {
                let v = app::VIEWS[d.selected() as usize].0;
                // A view another slot shows can't be chosen twice.
                if !others.iter().any(|o| o == v) {
                    tx.input(Msg::SetSwitch(k.clone(), json!({ slot.clone(): v })));
                }
            });
            view_rows.push(uuob_kit_gtk::setting_row(name, &drop));
        }
        let views = uuob_kit_gtk::group("Views", "What the switch on top offers. Phones and computers each keep their own.", view_rows);
        let before = self.screen["before"].as_i64();
        let tx = sender.clone();
        let current = before.map_or("none".to_string(), |b| b.to_string());
        let pick = alert_pick(app::alert_options(&current, before, false), current, move |v| tx.input(Msg::SetBefore(v.parse().ok())));
        pick.set_width_request(220);
        pick.set_hexpand(false);
        let alerts = uuob_kit_gtk::group("Alerts", "For events with a time; each event can have its own, any time before it up to a week. They ring on your phones and computers, as Calendar notifications.", vec![uuob_kit_gtk::setting_row("Default alert", &pick)]);
        let emails = self.emails();
        let mut inv_rows = vec![];
        if emails.is_empty() {
            inv_rows.push(uuob_kit_gtk::text_row("No address yet"));
        }
        for a in emails {
            let b = uuob_kit_gtk::icon_btn("close", 16, "Remove");
            let tx = sender.clone();
            let addr = a.clone();
            b.connect_clicked(move |_| tx.input(Msg::ForgetAddress(addr.clone())));
            inv_rows.push(uuob_kit_gtk::setting_row(&a, &b));
        }
        let inv = uuob_kit_gtk::group("Invitations", "Your addresses tell which guest is you. They're learned when you import or subscribe to your own Google calendar.", inv_rows);
        let tx = sender.clone();
        uuob_kit_gtk::settings_dialog(&self.info, &self.client, true, vec![views, alerts, inv], move || tx.input(Msg::Redraw)).present(Some(root));
    }
}

impl relm4::Component for Calendar {
    type Init = Client;
    type Input = Msg;
    type Output = ();
    type CommandOutput = Cmd;
    type Root = adw::ApplicationWindow;
    type Widgets = Widgets;

    fn init_root() -> Self::Root {
        adw::ApplicationWindow::builder().title("Calendar").default_width(1200).default_height(800).width_request(360).height_request(560).build()
    }

    fn init(client: Client, root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        uuob_kit_gtk::style();
        style();
        let info = AppInfo { id: "calendar".into(), name: "Calendar".into(), noun: "events".into(), icon: "x-office-calendar-symbolic".into(), color: "red".into(), version: calendar_core::version() };

        // The sidebar: calendars, other apps, Settings.
        let cals = gtk::ListBox::builder().css_classes(["navigation-sidebar"]).selection_mode(gtk::SelectionMode::None).build();
        let add = gtk::MenuButton::builder().child(&uuob_kit_gtk::icon("plus", 18, 2.2, "accent")).css_classes(["icon-btn", "add-cal"]).tooltip_text("Add a calendar").valign(gtk::Align::Center).build();
        // `.menu`: a small card of three, each with a blue icon.
        let items = gtk::Box::builder().orientation(gtk::Orientation::Vertical).css_classes(["web-menu"]).build();
        let pop = gtk::Popover::builder().child(&items).has_arrow(false).css_classes(["web-menu-pop"]).build();
        for (icon, text, which) in [("plus", "New calendar", 0), ("download", "Import a file", 1), ("link", "Subscribe to a link", 2)] {
            let row = gtk::Box::builder().spacing(10).build();
            row.append(&uuob_kit_gtk::icon(icon, 17, if which == 0 { 2.2 } else { 1.75 }, "accent"));
            row.append(&gtk::Label::new(Some(text)));
            let b = gtk::Button::builder().child(&row).css_classes(["web-menu-item"]).build();
            let (tx, pop) = (sender.clone(), pop.clone());
            b.connect_clicked(move |_| {
                pop.popdown();
                tx.input(match which {
                    0 => Msg::CalSheet(None),
                    1 => Msg::Import,
                    _ => Msg::Subscribe(String::new(), String::new()),
                })
            });
            items.append(&b);
        }
        add.set_popover(Some(&pop));
        let head = gtk::Box::builder().spacing(6).css_classes(["side-label"]).build();
        head.append(&gtk::Label::builder().label("CALENDARS").xalign(0.0).hexpand(true).css_classes(["section-label"]).build());
        head.append(&add);
        let tasks_check = gtk::CheckButton::builder().label("Tasks with dates").css_classes(["cal-row", "tasks-check"]).build();
        let tx = sender.clone();
        tasks_check.connect_toggled(move |_| tx.input(Msg::ToggleTasks));
        let settings = uuob_kit_gtk::btn("Settings", Some("settings"), &["settings-btn"]);
        settings.set_halign(gtk::Align::Start);
        settings.set_valign(gtk::Align::End);
        settings.set_vexpand(true);
        let tx = sender.clone();
        settings.connect_clicked(move |_| tx.input(Msg::Settings));
        let side = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(2).css_classes(["side-col"]).build();
        side.append(&head);
        side.append(&cals);
        side.append(&gtk::Label::builder().label("FROM OTHER APPS").xalign(0.0).css_classes(["section-label", "side-label", "other-apps"]).build());
        side.append(&tasks_check);
        side.append(&gtk::Label::builder().label("Asks to read your Tasks data.").xalign(0.0).margin_start(30).css_classes(["caption", "dim-label"]).build());
        side.append(&settings);
        let side_view = adw::ToolbarView::new();
        let sh = adw::HeaderBar::builder().show_title(false).build();
        sh.pack_start(&uuob_kit_gtk::update_button(FEED, info.version.clone()));
        side_view.add_top_bar(&sh);
        side_view.set_content(Some(&gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&side).build()));
        let side_page = adw::NavigationPage::builder().title("Calendars").child(&side_view).build();

        // The main part: the header, then the week or the month.
        let title = gtk::Label::builder().xalign(0.0).hexpand(true).ellipsize(gtk::pango::EllipsizeMode::End).css_classes(["large-title"]).build();
        let switch = gtk::Box::builder().spacing(2).valign(gtk::Align::Center).css_classes(["segmented"]).build();
        let prev = uuob_kit_gtk::icon_btn("left", 22, "Previous");
        let today = uuob_kit_gtk::btn("Today", None, &["sm"]);
        today.set_valign(gtk::Align::Center);
        let next = uuob_kit_gtk::icon_btn("right", 22, "Next");
        let newb = gtk::Button::builder().child(&uuob_kit_gtk::icon("plus", 20, 2.2, "#ffffff")).css_classes(["btn", "primary", "new"]).valign(gtk::Align::Center).tooltip_text("New event").build();
        for (b, which) in [(&prev, 0), (&today, 1), (&next, 2), (&newb, 3)] {
            let tx = sender.clone();
            b.connect_clicked(move |_| {
                tx.input(match which {
                    0 => Msg::Move(-1),
                    1 => Msg::Today,
                    2 => Msg::Move(1),
                    _ => Msg::NewEvent(String::new(), None, None),
                })
            });
        }
        let nav = gtk::Box::builder().spacing(2).valign(gtk::Align::Center).build();
        nav.append(&prev);
        nav.append(&today);
        nav.append(&next);
        let header = adw::HeaderBar::builder().show_title(false).build();
        // One row on computers; on phones the title and + first, the views and dates below
        // (the breakpoint further down moves them, as the web version's header wraps).
        let top = gtk::Grid::builder().column_spacing(14).row_spacing(8).css_classes(["cal-header"]).build();
        top.attach(&title, 0, 0, 1, 1);
        top.attach(&switch, 1, 0, 1, 1);
        top.attach(&nav, 2, 0, 1, 1);
        newb.set_halign(gtk::Align::End);
        top.attach(&newb, 3, 0, 1, 1);
        let heads = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let allday = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let hours = gtk::DrawingArea::builder().width_request(56).content_height((HOUR * 24.0) as i32).build();
        let columns = gtk::Box::builder().homogeneous(true).hexpand(true).build();
        let grid_row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        grid_row.append(&hours);
        grid_row.append(&columns);
        let grid_scroll = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&grid_row).build();
        let week = gtk::Box::new(gtk::Orientation::Vertical, 0);
        week.append(&heads);
        week.append(&allday);
        week.append(&gtk::Separator::builder().css_classes(["strong-line"]).build());
        week.append(&grid_scroll);
        let month = gtk::Grid::builder().column_homogeneous(true).vexpand(true).build();
        let body = gtk::Stack::new();
        body.add_named(&week, Some("week"));
        body.add_named(&gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&month).build(), Some("month"));
        let toast = adw::ToastOverlay::new();
        toast.set_child(Some(&body));
        let main_col = gtk::Box::builder().orientation(gtk::Orientation::Vertical).css_classes(["cal-main"]).build();
        main_col.append(&top);
        main_col.append(&toast);
        let main_view = adw::ToolbarView::new();
        main_view.add_top_bar(&header);
        main_view.set_content(Some(&main_col));
        let content_page = adw::NavigationPage::builder().title("Calendar").child(&main_view).build();
        let split = adw::NavigationSplitView::builder().sidebar(&side_page).content(&content_page).show_content(true).min_sidebar_width(240.0).max_sidebar_width(300.0).build();
        // Phones: the calendar list as its own page, and the phones' own switch.
        let narrow_bp = adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 720sp").expect("condition"));
        narrow_bp.add_setter(&split, "collapsed", Some(&true.to_value()));
        if let Some(lm) = top.layout_manager() {
            let at = |w: &gtk::Widget, prop: &str, v: i32| narrow_bp.add_setter(&lm.layout_child(w), prop, Some(&v.to_value()));
            at(title.upcast_ref(), "column-span", 3);
            at(switch.upcast_ref(), "row", 1);
            at(switch.upcast_ref(), "column", 0);
            at(nav.upcast_ref(), "row", 1);
            at(nav.upcast_ref(), "column", 1);
            at(nav.upcast_ref(), "column-span", 3);
        }
        narrow_bp.add_setter(&switch, "halign", Some(&gtk::Align::Start.to_value()));
        narrow_bp.add_setter(&nav, "halign", Some(&gtk::Align::End.to_value()));
        narrow_bp.add_setter(&title, "css-classes", Some(&["large-title", "narrow-title"].to_value()));
        narrow_bp.add_setter(&main_col, "css-classes", Some(&["cal-main", "narrow"].to_value()));
        let tx = sender.clone();
        narrow_bp.connect_apply(move |_| tx.input(Msg::Show("narrow".into())));
        let tx = sender.clone();
        narrow_bp.connect_unapply(move |_| tx.input(Msg::Show("wide".into())));
        root.add_breakpoint(narrow_bp);

        let gate = Gate::new(info.clone(), client.clone(), {
            let tx = sender.clone();
            move || tx.input(Msg::Reload)
        });
        gate.set_content(&split);
        root.set_content(Some(&gate.widget()));
        std::mem::forget(gate);

        // A switch between light and dark redraws what was drawn in the old colours.
        adw::StyleManager::default().connect_dark_notify({
            let tx = sender.clone();
            move |_| tx.input(Msg::Redraw)
        });
        let mut events = client.subscribe();
        let tx = sender.clone();
        glib::spawn_future_local(async move {
            while let Ok(e) = events.recv().await {
                match e {
                    Event::Changed(_) | Event::State(uuob_client::State::Connected) => tx.input(Msg::Reload),
                    Event::Settings(_) => tx.input(Msg::Redraw),
                    _ => {}
                }
            }
        });
        // The line for now moves on, and an app left open overnight rolls over.
        let tx = sender.clone();
        glib::timeout_add_seconds_local(30, move || {
            tx.input(Msg::Tick);
            glib::ControlFlow::Continue
        });
        let tx = sender.clone();
        glib::timeout_add_seconds_local(3600, move || {
            tx.input(Msg::Upcoming(0));
            glib::ControlFlow::Continue
        });
        let c = client.clone();
        glib::spawn_future_local(async move {
            let _ = app::move_old_calendars(&c).await;
        });

        // What was seen last, at once.
        let first = client.cached("calendars", None).and_then(|l| l.into_iter().find(|x| x["id"] == json!("personal"))).unwrap_or(Value::Null);
        let data = json!({
            "events": client.cached_all("events"), "infos": client.cached_all("calendar"),
            "containers": client.cached_containers().unwrap_or_default(), "first": first,
            "prefs": client.cached("prefs", None).unwrap_or_default(), "answers": client.cached("answers", None).unwrap_or_default(),
            "alerts": client.cached("alerts", None).unwrap_or_default(), "subs": client.cached("subscriptions", None).unwrap_or_default(),
        });
        let today_s = zones::today();
        let mut model = Calendar {
            client,
            info,
            data,
            view: String::new(),
            cursor: today_s.clone(),
            narrow: false,
            hidden: None,
            show_tasks: None,
            views: None,
            tasks: vec![],
            screen: Value::Null,
            events_known: false,
            prefs_known: false,
            refreshed_once: false,
            upcoming_gen: 0,
            shown_today: today_s,
        };
        let widgets = Widgets { root: root.clone(), cals, tasks_check, title, switch, body, heads, allday, columns, hours, grid_scroll, month, toast };
        model.render(&widgets, &sender);
        // Open on the morning, once the grid has its height (behind the connect screen it has none yet).
        let adj = widgets.grid_scroll.vadjustment();
        let done = Rc::new(std::cell::Cell::new(false));
        adj.connect_changed(move |a| {
            if !done.get() && a.upper() > HOUR * 8.0 && a.page_size() > 0.0 {
                done.set(true);
                let a = a.clone();
                glib::idle_add_local_once(move || a.set_value(HOUR * 7.5));
            }
        });
        ComponentParts { model, widgets }
    }

    fn update_with_view(&mut self, w: &mut Widgets, msg: Msg, sender: ComponentSender<Self>, root: &Self::Root) {
        let client = self.client.clone();
        let main = self.main_id();
        let morning = |w: &Widgets| {
            let gs = w.grid_scroll.clone();
            glib::idle_add_local_once(move || gs.vadjustment().set_value(HOUR * 7.5));
        };
        match msg {
            Msg::Reload => {
                sender.oneshot_command(async move {
                    let containers = client.containers().await.ok();
                    let (events, complete) = client.all("events").await;
                    let (infos, _) = client.all("calendar").await;
                    let first = client.load("calendars", None).await.ok().and_then(|l| l.into_iter().find(|x| x["id"] == json!("personal")));
                    let get = |r: Result<Vec<Value>, uuob_client::Error>| r.ok().map(Value::from);
                    let prefs = get(client.load("prefs", None).await);
                    let answers = get(client.load("answers", None).await);
                    let alerts = get(client.load("alerts", None).await);
                    let subs = get(client.load("subscriptions", None).await);
                    Cmd::Loaded(Box::new(json!({ "containers": containers, "events": events, "complete": complete, "infos": infos, "first": first, "prefs": prefs, "answers": answers, "alerts": alerts, "subs": subs })))
                });
                return;
            }
            Msg::Redraw => {}
            Msg::Tick => {
                let t = zones::today();
                if t != self.shown_today {
                    if self.cursor == self.shown_today {
                        self.cursor = t.clone(); // keep following "today" if we were on it
                    }
                    self.shown_today = t;
                }
            }
            Msg::Show(v) => {
                if v == "narrow" || v == "wide" {
                    self.narrow = v == "narrow";
                } else {
                    if v == self.view {
                        return;
                    }
                    self.view = v;
                    if self.view != "month" {
                        morning(w);
                    }
                }
            }
            Msg::Move(dir) => self.cursor = app::move_cursor(&self.view, &self.cursor, dir, uuob_kit_gtk::sunday_first(&self.client)),
            Msg::Today => {
                self.cursor = zones::today();
                morning(w);
            }
            Msg::Day(d) => {
                self.cursor = d;
                self.view = s(&self.screen, "dayView").to_string();
                morning(w);
            }
            Msg::NewEvent(date, start, end) => {
                let date = if !date.is_empty() {
                    date
                } else {
                    let t = zones::today();
                    let days = strs(&self.screen["days"]);
                    if self.view == "month" || days.contains(&t) { t } else { days.first().cloned().unwrap_or(t) }
                };
                let ed = app::new_event(&self.state(), &date, start.as_deref(), end.as_deref());
                return self.show_sheet(ed, root, &sender);
            }
            Msg::Open(key) => return self.show_card(&key, root, &sender),
            Msg::Edit(key) => {
                let (from, to) = self.range();
                let ed = app::edit_event(&self.state(), &key, &from, &to);
                if !ed.is_null() {
                    self.show_sheet(ed, root, &sender);
                }
                return;
            }
            Msg::Save(ed, scope) => {
                let series = ed["series"].is_object();
                let scope = match scope {
                    Some(sc) => Some(sc),
                    None if series && calendar_core::series::asks_scope(&ed) => return self.ask_scope(ed, false, root, &sender),
                    None if series => Some("all".into()),
                    None => None,
                };
                let events = arr(&self.data["events"]);
                sender.oneshot_command(async move {
                    match app::save_event(&client, &ed, scope.as_deref(), &events, &main).await {
                        Ok(()) => Cmd::Done,
                        Err(e) => Cmd::Error(e.to_string()),
                    }
                });
                return;
            }
            Msg::Delete(ed, scope) => {
                if ed["series"].is_object() && scope.is_none() {
                    return self.ask_scope(ed, true, root, &sender);
                }
                let events = arr(&self.data["events"]);
                sender.oneshot_command(async move {
                    match app::delete_event(&client, &ed, scope.as_deref(), &events, &main).await {
                        Ok(()) => Cmd::Done,
                        Err(e) => Cmd::Error(e.to_string()),
                    }
                });
                return;
            }
            Msg::Answer(key, a) => {
                let mut answers = arr(&self.data["answers"]);
                answers.retain(|x| s(x, "id") != key);
                answers.push(json!({ "id": key, "answer": a }));
                self.data["answers"] = json!(answers);
                sender.oneshot_command(async move {
                    let _ = app::set_answer(&client, &key, &a).await;
                    Cmd::Done
                });
                self.schedule_upcoming(&sender);
            }
            Msg::OwnAlert(key, choice) => {
                let mut alerts = arr(&self.data["alerts"]);
                alerts.retain(|x| s(x, "id") != key);
                if choice != "default" {
                    alerts.push(json!({ "id": key, "before": if choice == "none" { json!(false) } else { json!(choice.parse::<i64>().unwrap_or(0)) } }));
                }
                self.data["alerts"] = json!(alerts);
                sender.oneshot_command(async move {
                    let _ = app::set_own_alert(&client, &key, &choice).await;
                    Cmd::Done
                });
                self.schedule_upcoming(&sender);
                return;
            }
            Msg::ToggleCal(id) => {
                let mut h = strs(&self.screen["hidden"]);
                if h.contains(&id) {
                    h.retain(|x| *x != id);
                } else {
                    h.push(id);
                }
                self.hidden = Some(h.clone());
                let show = self.screen["showTasks"] == json!(true);
                sender.oneshot_command(async move {
                    let _ = app::save_view_prefs(&client, &h, show).await;
                    Cmd::Done
                });
            }
            Msg::ToggleTasks => {
                let on = w.tasks_check.is_active();
                if on == (self.screen["showTasks"] == json!(true)) {
                    return;
                }
                let h = strs(&self.screen["hidden"]);
                if on {
                    // Showing tasks needs read access to the Tasks app's container.
                    let tx = sender.clone();
                    let c = client.clone();
                    glib::spawn_future_local(async move {
                        match c.request_access("tasks", "read").await {
                            Ok(_) => {
                                let tasks = c.load("tasks", Some("tasks")).await.unwrap_or_default();
                                let _ = app::save_view_prefs(&c, &h, true).await;
                                tx.oneshot_command(std::future::ready(Cmd::Tasks(tasks)));
                            }
                            Err(e) => tx.input(Msg::Note(e.to_string())),
                        }
                    });
                    return;
                }
                self.show_tasks = Some(false);
                self.tasks.clear();
                sender.oneshot_command(async move {
                    let _ = app::save_view_prefs(&client, &h, false).await;
                    Cmd::Done
                });
            }
            Msg::CalSheet(c) => return self.show_cal_sheet(c, root, &sender),
            Msg::SaveCal(sheet) => {
                let infos = arr(&self.data["infos"]);
                let cals = arr(&self.screen["calendars"]);
                sender.oneshot_command(async move {
                    match app::save_calendar(&client, &sheet, &infos, &cals).await {
                        Ok(_) => Cmd::Done,
                        Err(e) => Cmd::Error(e.to_string()),
                    }
                });
                return;
            }
            Msg::DeleteCal(id, sub) => {
                sender.oneshot_command(async move {
                    match app::delete_calendar(&client, &id, sub).await {
                        Ok(_) => Cmd::Done,
                        Err(e) => Cmd::Error(e.to_string()),
                    }
                });
                return;
            }
            Msg::Share(id) => {
                if !client.remote() {
                    open_uri("uuob://share");
                }
                sender.oneshot_command(async move {
                    let _ = client.share(&id).await;
                    Cmd::Done
                });
                return;
            }
            Msg::Export(id) => {
                let name = arr(&self.screen["calendars"]).iter().find(|c| s(c, "id") == id).map(|c| s(c, "name").to_string()).unwrap_or_default();
                let (text, count) = app::export_calendar(&arr(&self.data["events"]), &id, &name);
                let file = app::export_name(&name);
                let dlg = gtk::FileDialog::builder().title("Save as .ics file").initial_name(&file).modal(true).build();
                let tx = sender.clone();
                dlg.save(Some(root), gio::Cancellable::NONE, move |r| {
                    if let Ok(f) = r {
                        let ok = f.path().is_some_and(|p| std::fs::write(&p, text.as_bytes()).is_ok());
                        tx.input(Msg::Note(if ok { format!("Saved {count} events as {file}.") } else { "The file could not be saved.".into() }));
                    }
                });
                return;
            }
            Msg::Note(text) => {
                w.toast.add_toast(adw::Toast::new(&text));
                return;
            }
            Msg::RefreshNow(id) => {
                let Some(sub) = arr(&self.data["subs"]).into_iter().find(|x| s(x, "id") == id) else { return };
                w.toast.add_toast(adw::Toast::new("Updating…"));
                let events = arr(&self.data["events"]);
                let mut emails = self.emails();
                let url = s(&sub, "url").to_string();
                let tx = sender.clone();
                glib::spawn_future_local(async move {
                    let r = app::refresh(&client, &id, &url, &events, &mut emails).await;
                    tx.input(Msg::Note(match r {
                        Ok(n) => format!("Updated: {n} events."),
                        Err(e) => e,
                    }));
                });
                return;
            }
            Msg::Subscribe(url, name) => {
                if url.is_empty() {
                    return self.show_subscribe(root, &sender);
                }
                w.toast.add_toast(adw::Toast::new("Reading the link…"));
                let n = arr(&self.screen["calendars"]).len();
                let mut emails = self.emails();
                let tx = sender.clone();
                glib::spawn_future_local(async move {
                    let r = app::subscribe(&client, &url, &name, n, &mut emails).await;
                    tx.oneshot_command(std::future::ready(Cmd::SubResult(r)));
                });
                return;
            }
            Msg::Import => {
                let filter = gtk::FileFilter::new();
                filter.set_name(Some("Calendars (.ics)"));
                filter.add_suffix("ics");
                filter.add_mime_type("text/calendar");
                let filters = gio::ListStore::new::<gtk::FileFilter>();
                filters.append(&filter);
                let dlg = gtk::FileDialog::builder().title("Import a file").filters(&filters).modal(true).build();
                let tx = sender.clone();
                dlg.open_multiple(Some(root), gio::Cancellable::NONE, move |r| {
                    let Ok(files) = r else { return };
                    let items: Vec<Value> = (0..files.n_items())
                        .filter_map(|i| files.item(i)?.downcast::<gio::File>().ok()?.path())
                        .map(|p| {
                            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                            match std::fs::read_to_string(&p) {
                                Ok(text) => app::read_file(&name, &text),
                                Err(_) => json!({ "name": name, "error": "This file could not be read as a calendar." }),
                            }
                        })
                        .collect();
                    if !items.is_empty() {
                        tx.input(Msg::ImportPick(items));
                    }
                });
                return;
            }
            Msg::ImportPick(items) => return self.show_import(items, root, &sender),
            Msg::ImportRun(items) => {
                w.toast.add_toast(adw::Toast::new("Importing…"));
                let n = arr(&self.screen["calendars"]).len();
                let mut emails = self.emails();
                let tx = sender.clone();
                glib::spawn_future_local(async move {
                    let r = app::import(&client, &items, n, &mut emails, |_| {}).await;
                    tx.oneshot_command(std::future::ready(Cmd::ImportResult(r)));
                });
                return;
            }
            Msg::Settings => return self.show_settings(root, &sender),
            Msg::SetSwitch(key, change) => {
                let mut views = self.screen["switch"].clone();
                for (k, v) in change.as_object().into_iter().flatten() {
                    views[key.as_str()][k.as_str()] = v.clone();
                }
                self.views = Some(views.clone());
                sender.oneshot_command(async move {
                    let _ = app::save_switch(&client, &views).await;
                    Cmd::Done
                });
            }
            Msg::SetBefore(b) => {
                let mut prefs = arr(&self.data["prefs"]);
                prefs.retain(|x| s(x, "id") != "alerts");
                prefs.push(json!({ "id": "alerts", "before": b }));
                self.data["prefs"] = json!(prefs);
                sender.oneshot_command(async move {
                    let _ = app::save_default_alert(&client, b).await;
                    Cmd::Done
                });
                self.schedule_upcoming(&sender);
                return;
            }
            Msg::ForgetAddress(a) => {
                let emails: Vec<String> = self.emails().into_iter().filter(|x| *x != a).collect();
                let mut prefs = arr(&self.data["prefs"]);
                prefs.retain(|x| s(x, "id") != "me");
                prefs.push(json!({ "id": "me", "emails": emails }));
                self.data["prefs"] = json!(prefs);
                sender.oneshot_command(async move {
                    let _ = app::save_emails(&client, &emails).await;
                    Cmd::Done
                });
            }
            Msg::Upcoming(0) => return self.schedule_upcoming(&sender),
            Msg::Upcoming(generation) => {
                if generation == self.upcoming_gen {
                    let list = app::upcoming_list(&self.state());
                    glib::spawn_future_local(async move { client.upcoming(list).await });
                }
                return;
            }
        }
        self.render(w, &sender);
    }

    fn update_cmd_with_view(&mut self, w: &mut Widgets, cmd: Cmd, sender: ComponentSender<Self>, _: &Self::Root) {
        match cmd {
            Cmd::Loaded(d) => {
                if let Some(c) = d["containers"].as_array() {
                    self.data["containers"] = json!(c);
                }
                self.data["events"] = d["events"].clone();
                self.data["infos"] = d["infos"].clone();
                if !d["first"].is_null() {
                    self.data["first"] = d["first"].clone();
                }
                for k in ["prefs", "answers", "alerts", "subs"] {
                    if d[k].is_array() {
                        self.data[k] = d[k].clone();
                    }
                }
                self.events_known |= d["complete"] == json!(true);
                if d["prefs"].is_array() {
                    let first = !self.prefs_known;
                    self.prefs_known = true;
                    self.hidden = None;
                    self.views = None;
                    if first && app::prefs_of(&arr(&d["prefs"]))["show_tasks"] == json!(true) {
                        // Tasks show: read them (access was given before).
                        let c = self.client.clone();
                        sender.oneshot_command(async move { Cmd::Tasks(c.load("tasks", Some("tasks")).await.unwrap_or_default()) });
                    }
                }
                self.schedule_upcoming(&sender);
                // Calendar links are read again when the app opens, at most every few hours.
                if !self.refreshed_once && d["subs"].is_array() && d["containers"].is_array() {
                    self.refreshed_once = true;
                    let due = app::due_subscriptions(&arr(&d["subs"]), &arr(&d["containers"]), zones::now_ms());
                    if !due.is_empty() {
                        let client = self.client.clone();
                        let events = arr(&self.data["events"]);
                        let mut emails = self.emails();
                        glib::spawn_future_local(async move {
                            for sub in due {
                                let _ = app::refresh(&client, s(&sub, "id"), s(&sub, "url"), &events, &mut emails).await;
                            }
                        });
                    }
                }
            }
            Cmd::Tasks(t) => {
                self.tasks = t;
                self.show_tasks = Some(true);
            }
            Cmd::Done => return,
            Cmd::Error(e) => {
                w.toast.add_toast(adw::Toast::new(&e));
                return;
            }
            Cmd::SubResult(r) => {
                w.toast.add_toast(adw::Toast::new(&r.map(|_| "Subscribed.".to_string()).unwrap_or_else(|e| e)));
                return;
            }
            Cmd::ImportResult(r) => {
                let mut text = format!("Imported {} {}.", r["events"], if r["events"] == json!(1) { "event" } else { "events" });
                if let Some(e) = r["error"].as_str() {
                    text.push_str(&format!(" {e} Importing the file again picks up where it stopped."));
                }
                let n = r["unreadable"].as_i64().unwrap_or(0);
                if n > 0 {
                    text.push_str(&format!(" {n} {} in a way this calendar can't show yet; they appear only the first time.", if n == 1 { "event repeats" } else { "events repeat" }));
                }
                let z = strs(&r["zones"]);
                if !z.is_empty() {
                    text.push_str(&format!(" Times in {} were read in the calendar's own time zone.", z.join(", ")));
                }
                let d = uuob_kit_gtk::confirm_sheet("Import", &format!("{text}\n\nImporting the same file again updates these events instead of adding them twice."), &[("Done", &["primary"], "ok")], false, |_| {});
                d.present(Some(&w.root));
                return;
            }
        }
        self.render(w, &sender);
    }
}

fn main() {
    let client = Client::new(App { id: "calendar".into(), name: "Calendar".into(), android_package: None }, data_dir());
    RelmApp::new("app.uuob.calendar").with_args(Vec::new()).run::<Calendar>(client);
}
