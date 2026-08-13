export class ApiError extends Error {
  constructor(status, code, message, details) {
    super(message || code || `HTTP ${status}`);
    this.status = status;
    this.code = code;
    this.details = details;
  }
}

export async function api(path, opts = {}) {
  const res = await fetch(path, opts);
  if (res.status === 24) return null; // safety
  if (res.status === 204) return null;
  const text = await res.text();
  let json = null;
  try {
    json = text ? JSON.parse(text) : null;
  } catch {}

  if (!res.ok) {
    const err = json?.error || {};
    throw new ApiError(
      res.status,
      err.code || "unknown",
      err.message,
      err.details,
    );
  }
  return json;
}

export function shareUrl(id) {
  return `/f/${id}`;
}
