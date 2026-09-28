import type { Detection, QuotaBucket } from "./ApplicationInventory";
import type { Locale, View } from "./MacShell";

type Props = {
  locale: Locale;
  applications: Detection[];
  connected: [string, string][];
  quotaBuckets: QuotaBucket[];
  running: boolean;
  firstRun: boolean;
  onLeaveFirstRun: () => void;
  onView: (view: View) => void;
};

export default function Overview({ locale, applications, connected, quotaBuckets, running, firstRun, onLeaveFirstRun, onView }: Props) {
  const zh = locale === "zh";
  const found = new Set(applications.map(item => item.app_id)).size;
  const names = new Map(applications.map(item => [item.app_id, item.name]));
  const nextView: View = connected.length ? "api" : "discover";
  const nextTitle = connected.length ? zh ? "配置本机 API" : "Configure the local API" : zh ? "添加一个官方运行时" : "Add an official runtime";
  const nextText = connected.length ? zh ? "已有保存的连接。启动本机服务后，使用当前 Key 选择实际报告的模型并复制配置。" : "A connection is saved. Start the local service, choose a reported model for the current key, then copy its configuration." : zh ? "先从已扫描的应用中选择官方 CLI；添加后再查看报告的模型与额度。" : "Choose an official CLI from the scanned apps, then inspect its reported models and quota.";

  return <section className="page home-page">
    <header className="page-heading"><div><h1>{zh ? "总览" : "Overview"}</h1><p>{zh ? "扫描已安装的 AI 应用，显式连接官方运行时，再复制本机 API 配置。" : "Find installed AI apps, explicitly connect an official runtime, then copy a local API configuration."}</p></div></header>
    {firstRun && <section className="notice"><strong>{zh ? "首次使用" : "First run"}</strong><p>{zh ? "发现应用 → 添加官方运行时 → 启动本机 API" : "Find apps → Add an official runtime → Start the local API"}</p><button onClick={() => { onView("discover"); onLeaveFirstRun(); }}>{zh ? "查看应用" : "View apps"}</button><button className="quiet" onClick={onLeaveFirstRun}>{zh ? "稍后" : "Later"}</button></section>}
    <section className="overview-strip" aria-label={zh ? "当前状态" : "Current status"}><div><span>{zh ? "发现应用" : "Found apps"}</span><strong>{found}</strong></div><div><span>{zh ? "已保存连接" : "Saved connections"}</span><strong>{connected.length}</strong></div><div><span>{zh ? "本地 API" : "Local API"}</span><strong>{running ? zh ? "运行中" : "Running" : zh ? "未启动" : "Stopped"}</strong></div></section>
    <div className="overview-body"><section className="overview-section"><div className="overview-section-head"><h2>{zh ? "连接概览" : "Connections"}</h2><button className="text-action" onClick={() => onView("discover")}>{zh ? "查看全部应用" : "View all apps"}</button></div>{connected.length ? <div className="overview-connections">{connected.map(([id, path]) => {
      const buckets = quotaBuckets.filter(item => item.connection_id === id);
      return <div className="overview-connection" key={id}><div><strong>{path.startsWith("os-credential:") ? "Xiaomi MiMo API" : names.get(id) ?? id}</strong><span>{zh ? "已保存连接 · 运行状态待检" : "Saved connection · Runtime status unchecked"}</span></div><small>{buckets.length ? zh ? `${buckets.length} 个已报告额度窗口` : `${buckets.length} reported quota windows` : zh ? "额度未知" : "Quota unknown"}</small></div>;
    })}</div> : <p className="empty-state">{zh ? "尚无连接。发现应用不代表已经获得 API 调用能力。" : "No connection yet. Detection does not prove API access."}</p>}</section>
      <section className="overview-section next-step"><h2>{zh ? "下一步" : "Next step"}</h2><strong>{nextTitle}</strong><p>{nextText}</p><button className="primary" onClick={() => onView(nextView)}>{connected.length ? zh ? "打开 API" : "Open API" : zh ? "查看应用" : "View apps"}</button></section></div>
  </section>;
}
