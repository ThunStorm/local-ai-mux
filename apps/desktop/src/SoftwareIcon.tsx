const names: Record<string, string> = { codex: "Codex", qoder: "Qoder", "qoder-cn": "Qoder CN", mimo: "MiMo", mimocode: "MiMo Code", codebuddy: "CodeBuddy Code", workbuddy: "WorkBuddy", "codebuddy-cn": "CodeBuddy CN" };
const originalAssets = new Set(["codex", "qoder", "qoder-cn", "mimo", "codebuddy", "workbuddy"]);
export default function SoftwareIcon({ id, name, small = false }: { id: string; name?: string; small?: boolean }) {
  const asset = id === "mimocode" ? "mimo" : id;
  const label = name ?? names[id] ?? id;
  return originalAssets.has(asset) ? <img className={`software-icon${small ? " small" : ""}`} src={`/software-icons/${asset}.png`} alt="" aria-hidden="true" title={`${label} · 原应用图标`} /> : <span className={`software-icon fallback${small ? " small" : ""}`} aria-hidden="true" title={`${label} · 原应用图标暂未取得`}><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8"><rect x="3" y="4" width="18" height="16" rx="4" /><path d="m7 9 3 3-3 3m6 0h4" /></svg></span>;
}
