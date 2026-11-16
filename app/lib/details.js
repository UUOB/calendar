// What the event card shows besides the times: the meeting link to join, the
// notes without the joining instructions Google and Teams paste in, links in
// the notes, the people invited and your own answer.
//
// Only what is shown changes: the event's record keeps its notes as they are.

/** Video meeting services, by the host of their links. */
const SERVICES = [
  [/(^|\.)meet\.google\.com$/, 'Google Meet'],
  [/(^|\.)zoom\.us$/, 'Zoom'],
  [/^teams\.(microsoft|live)\.com$/, 'Microsoft Teams'],
  [/(^|\.)webex\.com$/, 'Webex'],
  [/^meet\.jit\.si$/, 'Jitsi Meet'],
  [/(^|\.)whereby\.com$/, 'Whereby'],
];
const URL_RE = /\b(?:https?:\/\/|mailto:|tel:)[^\s<>"'()]+[^\s<>"'().,;:!?]/gi;

function service(url) {
  try {
    const host = new URL(url).hostname.toLowerCase();
    return SERVICES.find(([re]) => re.test(host))?.[1] ?? null;
  } catch {
    return null;
  }
}

/** The video meeting to join, if the event has one: { url, name, host }. */
export function meeting(e) {
  const found = [e.conference, e.location, e.url, plain(e.notes)]
    .filter(Boolean)
    .flatMap((s) => String(s).match(URL_RE) ?? [])
    .find((u) => service(u));
  if (!found) return null;
  return { url: found, name: service(found), host: new URL(found).hostname.replace(/^www\./, '') };
}

/** Notes as plain text: Google sometimes stores them as HTML. */
export function plain(notes) {
  const s = String(notes ?? '');
  if (!/<(br|p|div|a|b|i|u|ul|ol|li|span)\b[^>]*>/i.test(s)) return s;
  return s
    .replace(/<a\b[^>]*href="([^"]*)"[^>]*>(.*?)<\/a>/gi, (_, href, text) => (text.replace(/<[^>]+>/g, '').trim() === href || !text.trim() ? href : `${text} (${href})`))
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<\/(p|div|li)>/gi, '\n')
    .replace(/<li\b[^>]*>/gi, '• ')
    .replace(/<[^>]+>/g, '')
    .replace(/&nbsp;/g, ' ')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;|&apos;/g, "'")
    .replace(/&amp;/g, '&')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}

/**
 * The notes split into what someone wrote and the joining instructions a
 * meeting service added (Google's block between "-::~:~::~" lines, Teams'
 * block between lines of underscores): { text, joining }.
 */
export function notesOf(notes) {
  const lines = plain(notes).split('\n');
  const keep = [];
  const joining = [];
  let block = null; // { kind, lines } while inside one; folded only once it closes
  for (const line of lines) {
    const t = line.trim();
    const kind = /^-::~[:~]+-?$/.test(t) ? 'google' : /^_{20,}$/.test(t) ? 'teams' : null;
    if (block) {
      if (kind === block.kind) (joining.push(...block.lines.slice(1)), (block = null));
      else block.lines.push(line);
    } else if (kind) block = { kind, lines: [line] };
    else keep.push(line);
  }
  if (block) keep.push(...block.lines); // a divider that never closes is just text
  const tidy = (l) => l.join('\n').replace(/\n{3,}/g, '\n\n').trim();
  return { text: tidy(keep), joining: tidy(joining.filter((l) => !/please do not edit this section/i.test(l))) };
}

/** Text as pieces to show, with links: [{ text } | { text, href }]. */
export function linkify(text) {
  const out = [];
  let at = 0;
  for (const m of String(text ?? '').matchAll(URL_RE)) {
    if (m.index > at) out.push({ text: text.slice(at, m.index) });
    const href = m[0];
    out.push({ text: href.replace(/^(mailto|tel):/i, '').replace(/^https?:\/\/(www\.)?/i, ''), href });
    at = m.index + href.length;
  }
  if (at < String(text ?? '').length) out.push({ text: text.slice(at) });
  return out;
}

// ------------------------------------------------------------ people

const lower = (s) => String(s ?? '').trim().toLowerCase();
/** Whether `text` is an email address (Google names a person's calendar by theirs). */
export const isEmail = (text) => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(String(text ?? '').trim());

/**
 * The people of an event, organizer first, then by answer: each
 * { name, email, answer: 'yes'|'maybe'|'no'|null, organizer, you }.
 * `mine` is the set of your email addresses; `answer` your own answer, which
 * replaces the one in the file.
 */
export function peopleOf(e, mine = new Set(), answer = null) {
  const list = (e.attendees ?? []).map((a) => ({ ...a, organizer: false, you: mine.has(lower(a.email)) }));
  const org = e.organizer;
  if (org?.email) {
    const at = list.find((a) => lower(a.email) === lower(org.email));
    if (at) at.organizer = true;
    else if (list.length) list.push({ name: org.name, email: org.email, answer: 'yes', organizer: true, you: mine.has(lower(org.email)) });
  }
  for (const p of list) if (p.you && answer) p.answer = answer;
  const rank = (p) => (p.organizer ? 0 : { yes: 1, maybe: 2, no: 3 }[p.answer] ?? 4);
  return list.sort((a, b) => rank(a) - rank(b) || (a.name || a.email || '').localeCompare(b.name || b.email || ''));
}

/** "30 people · 4 yes · 2 maybe · 2 no" */
export function peopleSummary(people) {
  const n = (x) => people.filter((p) => p.answer === x).length;
  return [`${people.length} ${people.length === 1 ? 'person' : 'people'}`, ...[['yes', n('yes')], ['maybe', n('maybe')], ['no', n('no')]].filter(([, c]) => c).map(([w, c]) => `${c} ${w}`)].join(' · ');
}

/**
 * Your answer to an invitation: what you chose here, else what the file says
 * for your address. 'yes' | 'maybe' | 'no' | 'invited' (not answered yet), or
 * null when the event isn't an invitation to you (or you organize it).
 */
export function answerOf(e, mine, chosen) {
  if (!e.attendees?.length) return null;
  if (e.organizer?.email && mine.has(lower(e.organizer.email))) return null;
  if (chosen) return chosen;
  const me = e.attendees.find((a) => mine.has(lower(a.email)));
  if (!me) return null;
  return me.answer ?? 'invited';
}

/** Whether to offer Going / Maybe / No: an invitation you don't organize. */
export const asksAnswer = (e, mine) => !!e.attendees?.length && !(e.organizer?.email && mine.has(lower(e.organizer.email)));
