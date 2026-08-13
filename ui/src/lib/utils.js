export function formatBytes(bytes) {
  if (bytes === null || bytes === undefined) return "-";
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(i === 0 ? 0 : 1)} ${sizes[i]}`;
}

export function formatCountdown(expiresAtSec, nowSec) {
  if (!expiresAtSec) return null;
  const rem = Math.max(0, Math.floor(expiresAtSec - nowSec));
  if (rem === 0) return "EXPIRED";
  const h = Math.floor(rem / 3600);
  const m = Math.floor((rem % 3600) / 60);
  const s = rem % 60;
  if (h > 0) return `${h}h ${m}m ${s}s`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

export function formatUTC(seconds) {
  if (!seconds) return "-";
  const d = new Date(seconds * 1000);
  return d.toISOString().replace("T", " ").replace(/\.\d{3}Z$/, " UTC");
}
