// The two things the web view can't do on its own: fetch a calendar link from
// another site (most block it), and save a file where the user picks. In the
// app, Tauri's plugins do them; in a plain browser (vite dev) the browser's
// own ways are tried instead.

const inApp = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
const MAX = 20 * 1024 * 1024; // a calendar file bigger than this isn't a calendar

/** "webcal://…" and "http://…" -> "https://…"; anything else isn't a calendar link. */
export function calendarUrl(text) {
  let u = String(text ?? '').trim();
  if (/^webcals?:\/\//i.test(u)) u = u.replace(/^webcals?:/i, 'https:');
  if (/^http:\/\//i.test(u)) u = u.replace(/^http:/i, 'https:');
  if (!/^https:\/\//i.test(u)) u = `https://${u.replace(/^\/+/, '')}`;
  try {
    const url = new URL(u);
    return url.hostname.includes('.') ? url.href : null;
  } catch {
    return null;
  }
}

/** The text at a calendar link (https only). */
export async function fetchText(url) {
  const get = inApp ? (await import('@tauri-apps/plugin-http')).fetch : window.fetch.bind(window);
  const res = await get(url, { method: 'GET', headers: { Accept: 'text/calendar, */*' }, ...(inApp ? { connectTimeout: 20000 } : {}) });
  if (!res.ok) throw new Error(res.status === 404 ? 'Nothing was found at that link.' : `The link answered ${res.status}.`);
  const text = await res.text();
  if (text.length > MAX) throw new Error('That file is too big to be a calendar.');
  if (!/BEGIN:VCALENDAR/i.test(text.slice(0, 2000))) throw new Error('That link is not a calendar (.ics).');
  return text;
}

/** Saves `text` as a file the user names and places. False if they cancelled. */
export async function saveText(name, text) {
  if (inApp) {
    const { save } = await import('@tauri-apps/plugin-dialog');
    const { writeTextFile } = await import('@tauri-apps/plugin-fs');
    const path = await save({ defaultPath: name, filters: [{ name: 'Calendar', extensions: ['ics'] }] });
    if (!path) return false;
    await writeTextFile(path, text);
    return true;
  }
  const a = document.createElement('a');
  a.href = URL.createObjectURL(new Blob([text], { type: 'text/calendar' }));
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 10000);
  return true;
}
