import type { LiveSession } from "./session";
import s from "./TerminalKeys.module.css";

const keys = [
  ["Esc", "\x1b"], ["Tab", "\t"], ["Ctrl+C", "\x03"], ["Ctrl+D", "\x04"],
  ["↑", "\x1b[A"], ["↓", "\x1b[B"], ["←", "\x1b[D"], ["→", "\x1b[C"],
  ["Home", "\x1b[H"], ["End", "\x1b[F"], ["PgUp", "\x1b[5~"], ["PgDn", "\x1b[6~"],
] as const;

/** Touch keyboards lack terminal control keys. Input uses the existing shared session path. */
export function TerminalKeys({ session, connected }: { session: LiveSession; connected: boolean }) {
  return <div className={s.keys}>
    {keys.map(([label, data]) => <button key={label} type="button" disabled={!connected}
      onPointerDown={(event) => event.preventDefault()}
      onClick={() => { session.focus(); void session.sendInput(data).catch(() => {}); }}>{label}</button>)}
  </div>;
}
