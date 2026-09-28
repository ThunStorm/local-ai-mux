import { useEffect, useRef, type ReactNode } from "react";

export type View = "home" | "discover" | "routing" | "sessions" | "api";
export type Locale = "zh" | "en";
export type Theme = "system" | "light" | "dark";

type Props = {
  view: View;
  onView: (view: View) => void;
  locale: Locale;
  onLocale: (locale: Locale) => void;
  theme: Theme;
  onTheme: (theme: Theme) => void;
  version: string;
  children: ReactNode;
};

function NavIcon({ name }: { name: View }) {
  const paths: Record<View, ReactNode> = {
    home: <><path d="m3 10 9-7 9 7v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1z" /><path d="M9 21v-7h6v7" /></>,
    discover: <><rect x="3" y="3" width="7" height="7" rx="1.5" /><rect x="14" y="3" width="7" height="7" rx="1.5" /><rect x="3" y="14" width="7" height="7" rx="1.5" /><rect x="14" y="14" width="7" height="7" rx="1.5" /></>,
    api: <><path d="m8 7-5 5 5 5M16 7l5 5-5 5M14 4l-4 16" /></>,
    routing: <><circle cx="6" cy="5" r="2" /><circle cx="18" cy="12" r="2" /><circle cx="6" cy="19" r="2" /><path d="M8 5h5a5 5 0 0 1 5 5M8 19h5a5 5 0 0 0 5-5" /></>,
    sessions: <><path d="M4 5h16v12H8l-4 3z" /><path d="M8 9h8M8 13h5" /></>,
  };
  return <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">{paths[name]}</svg>;
}

export default function MacShell({ view, onView, locale, onLocale, theme, onTheme, version, children }: Props) {
  const content = useRef<HTMLDivElement>(null);
  const preferences = useRef<HTMLDetailsElement>(null);
  useEffect(() => { if (content.current) content.current.scrollTop = 0; if (preferences.current) preferences.current.open = false; }, [view]);
  const zh = locale === "zh";
  const items: { id: View; zh: string; en: string }[] = [
    { id: "home", zh: "总览", en: "Overview" },
    { id: "discover", zh: "应用与连接", en: "Apps & connections" },
    { id: "api", zh: "API", en: "API" },
  ];
  const advanced: typeof items = [
    { id: "routing", zh: "路由", en: "Routing" },
    { id: "sessions", zh: "会话", en: "Sessions" },
  ];
  const navButton = (item: typeof items[number]) => {
    const label = zh ? item.zh : item.en;
    return <button key={item.id} type="button" className={`sidebar-link${view === item.id ? " selected" : ""}`} title={label} aria-label={label} aria-current={view === item.id ? "page" : undefined} onClick={() => onView(item.id)}><NavIcon name={item.id} /><span className="nav-label">{label}</span></button>;
  };

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="sidebar-brand"><span className="brand-monogram" aria-hidden="true">L</span><strong>LocalMux</strong><span className="brand-subtitle">智汇枢</span></div>
      <nav aria-label={zh ? "主导航" : "Main navigation"}>{items.map(navButton)}<div className="sidebar-section">{zh ? "高级" : "Advanced"}</div>{advanced.map(navButton)}</nav>
      <div className="sidebar-bottom">
        <details className="sidebar-preferences" ref={preferences} onKeyDown={event => { if (event.key === "Escape" && preferences.current) preferences.current.open = false; }}><summary aria-label={zh ? "偏好设置" : "Preferences"} title={zh ? "偏好设置" : "Preferences"}><svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3" /><path d="m10 2-.5 2.2-2 1-2.1-.7-2 3.5 1.6 1.6v2.4L3.4 14l2 3.5 2.1-.7 2 1L10 20h4l.5-2.2 2-1 2.1.7 2-3.5-1.6-1.6V10l1.6-1.5-2-3.5-2.1.7-2-1L14 2z" /></svg><span>{zh ? "偏好设置" : "Preferences"}</span></summary><div className="preferences-menu"><label>{zh ? "语言" : "Language"}<select aria-label={zh ? "语言" : "Language"} value={locale} onChange={event => onLocale(event.target.value as Locale)}><option value="zh">中文</option><option value="en">English</option></select></label><label>{zh ? "外观" : "Appearance"}<select aria-label={zh ? "外观" : "Appearance"} value={theme} onChange={event => onTheme(event.target.value as Theme)}><option value="system">{zh ? "跟随系统" : "System"}</option><option value="light">{zh ? "浅色" : "Light"}</option><option value="dark">{zh ? "深色" : "Dark"}</option></select></label></div></details>
        <small>v{version}</small>
      </div>
    </aside>
    <main className="workspace"><div className="workspace-scroll" ref={content}>{children}</div></main>
  </div>;
}
