import type { QuotaBucket } from "./ApplicationInventory";

// Both views consume the same bucket list, including independent observation/reset times.
export default function QuotaLines({ buckets, locale, details = false }: { buckets: QuotaBucket[]; locale: "zh" | "en"; details?: boolean }) {
  const zh = locale === "zh";
  const now = Date.now() / 1000;
  const date = (time: number) => new Date(time * 1000).toLocaleString(zh ? "zh-CN" : "en-US");
  return <div className="quota-lines">{buckets.length === 0 ? <small>{zh ? "额度未知" : "Quota unknown"}</small> : buckets.map(bucket => {
    const fresh = bucket.observed_at > 0 && bucket.observed_at <= now && now - bucket.observed_at <= 600;
    const reported = bucket.source === "PROVIDER_REPORTED";
    const value = reported && bucket.remaining !== null ? `${zh ? "剩余" : "Remaining"} ${bucket.remaining}${bucket.unit === "%" ? "%" : ` / ${bucket.limit ?? (zh ? "未知" : "Unknown")} ${bucket.unit ?? ""}`}` : zh ? "未知" : "Unknown";
    const label = bucket.bucket_id === "legacy" ? zh ? "上游额度" : "Upstream quota" : zh ? bucket.label === "primary" ? "主窗口" : bucket.label === "secondary" ? "次窗口" : bucket.label.replace(" min window", " 分钟窗口") : bucket.label;
    const source = reported ? zh ? "上游报告" : "Provider reported" : bucket.source === "ESTIMATED" ? zh ? "估算" : "Estimated" : zh ? "未知" : "Unknown";
    return <small key={bucket.bucket_id}><span>{label} · {value}{!fresh && (zh ? " · 已过期" : " · Stale")}</span>{details && <> · {source}{bucket.observed_at > 0 && ` · ${zh ? "上次观测" : "Observed"} ${date(bucket.observed_at)}`}{bucket.reset_at !== null && ` · ${zh ? "重置" : "Resets"} ${date(bucket.reset_at)}`}</>}</small>;
  })}</div>;
}
