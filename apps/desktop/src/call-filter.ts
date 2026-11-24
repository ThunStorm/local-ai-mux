export function localTimeRange(from: string, before: string): { started_from: number | null; started_before: number | null } | null {
  const parse = (value: string): number | null => {
    if (!value) return null;
    if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(value)) return NaN;
    const date = new Date(value);
    if (!Number.isFinite(date.getTime()) || date.getTime() < 0) return NaN;
    const [y, m, d, h, min] = value.split(/[-T:]/).map(Number);
    if (date.getFullYear() !== y || date.getMonth() + 1 !== m || date.getDate() !== d || date.getHours() !== h || date.getMinutes() !== min) return NaN;
    return Math.floor(date.getTime() / 1000);
  };
  const started_from = parse(from), started_before = parse(before);
  if ((started_from !== null && !Number.isFinite(started_from)) || (started_before !== null && !Number.isFinite(started_before)) || (started_from !== null && started_before !== null && started_from >= started_before)) return null;
  return { started_from, started_before };
}
export const exportError = (code: unknown, zh: boolean): string => {
  const messages: Record<string, [string, string]> = {
    INVALID_TIME_RANGE: ["时间段无效，结束时间必须晚于开始时间。", "Invalid time range; end must be after start."],
    EXPORT_TOO_LARGE: ["匹配记录超过 10,000 条或 64 MiB，请缩小筛选范围。", "More than 10,000 calls or 64 MiB matched. Narrow the filters."],
    EXPORT_BUSY: ["已有导出正在进行，请等待完成。", "An export is already in progress."],
    EXPORT_EXTENSION_MISMATCH: ["文件扩展名与所选格式不一致，请重试。", "The extension does not match the format. Retry."],
    EXPORT_WRITE_FAILED: ["保存失败，请检查目录权限或可用空间；原文件未被覆盖。", "Save failed. Check permissions or free space; the original file was preserved."],
  };
  return messages[String(code)]?.[zh ? 0 : 1] ?? (zh ? "导出失败，请重试或缩小筛选范围。" : "Export failed. Retry or narrow the filters.");
};
