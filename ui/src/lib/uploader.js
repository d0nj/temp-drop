import { api, ApiError, shareUrl } from "./api.js";
import { generateKey, exportKeyToString, encryptChunk, encryptText } from "./crypto.js";

const RETRIES = 3;

function uploadChunkXHR({ url, headers, body, signal, onChunkProgress }) {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open("PUT", url);

    if (headers) {
      for (const [k, v] of Object.entries(headers)) {
        xhr.setRequestHeader(k, v);
      }
    }

    if (signal) {
      if (signal.aborted) {
        return reject(new DOMException("Aborted", "AbortError"));
      }
      signal.addEventListener("abort", () => {
        xhr.abort();
        reject(new DOMException("Aborted", "AbortError"));
      });
    }

    if (xhr.upload && onChunkProgress) {
      xhr.upload.onprogress = (e) => {
        if (e.lengthComputable) {
          onChunkProgress(e.loaded, e.total);
        }
      };
    }

    xhr.onload = () => {
      if (xhr.status >= 200 && xhr.status < 300) {
        resolve(xhr);
      } else {
        const err = new Error(`HTTP ${xhr.status} ${xhr.statusText}`);
        err.status = xhr.status;
        err.responseText = xhr.responseText;
        reject(err);
      }
    };

    xhr.onerror = () => reject(new Error("Network error during chunk upload"));
    xhr.ontimeout = () => reject(new Error("Timeout during chunk upload"));

    xhr.send(body);
  });
}

export async function uploadFile({
  file,
  chunkSize,
  ttlSeconds,
  maxDownloads,
  signal,
  onProgress,
}) {
  // Generate random AES-256-GCM encryption key
  const cryptoKey = await generateKey();
  const keyStr = await exportKeyToString(cryptoKey);

  // Encrypt filename client-side before sending to server
  const encryptedName = "enc:" + (await encryptText(file.name || "unnamed", cryptoKey));

  const req = {
    name: encryptedName,
    size_bytes: file.size,
    ttl_seconds: ttlSeconds ?? null,
    max_downloads: maxDownloads ?? null,
  };
  const started = await api("/api/uploads", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(req),
    signal,
  });
  const id = started.id;
  const token = started.upload_token;
  let etags = [];

  try {
    for (let n = 1; ; n++) {
      const startByte = (n - 1) * chunkSize;
      if (startByte >= file.size) break;
      const rawChunkSlice = file.slice(
        startByte,
        Math.min(startByte + chunkSize, file.size),
      );
      const rawBuffer = await rawChunkSlice.arrayBuffer();

      // Encrypt chunk bytes: result is [12-byte IV][Ciphertext + Tag]
      const encryptedBytes = await encryptChunk(rawBuffer, cryptoKey);

      let attempt = 0;
      let ok = false;
      while (!ok) {
        attempt++;
        try {
          if (started.backend === "s3") {
            const presigned = await api(`/api/uploads/${id}/parts/${n}/presign`, {
              headers: { "x-upload-token": token },
              signal,
            });
            const xhr = await uploadChunkXHR({
              url: presigned.url,
              body: encryptedBytes,
              signal,
              onChunkProgress: (loaded, total) => {
                const ratio = total > 0 ? Math.min(1, loaded / total) : 1;
                const rawSentInChunk = Math.round(ratio * rawChunkSlice.size);
                const totalSent = Math.min(file.size, startByte + rawSentInChunk);
                onProgress?.(totalSent, file.size, n);
              },
            });
            const etag = xhr.getResponseHeader("etag");
            if (!etag) throw new Error(`presigned part ${n}: no etag`);
            etags.push(etag);
          } else {
            await uploadChunkXHR({
              url: `/api/uploads/${id}/parts/${n}`,
              headers: {
                "x-upload-token": token,
                "content-type": "application/octet-stream",
              },
              body: encryptedBytes,
              signal,
              onChunkProgress: (loaded, total) => {
                const ratio = total > 0 ? Math.min(1, loaded / total) : 1;
                const rawSentInChunk = Math.round(ratio * rawChunkSlice.size);
                const totalSent = Math.min(file.size, startByte + rawSentInChunk);
                onProgress?.(totalSent, file.size, n);
              },
            });
          }
          ok = true;
        } catch (e) {
          if (
            e instanceof ApiError &&
            [400, 401, 404, 409, 413, 422, 507].includes(e.status ?? -1)
          ) {
            throw e;
          }
          if (attempt >= RETRIES) throw e;
        }
      }
      const finishedSent = Math.min(file.size, startByte + rawChunkSlice.size);
      onProgress?.(finishedSent, file.size, n);
    }
    if (started.backend === "s3") {
      await api(`/api/uploads/${id}/complete`, {
        method: "POST",
        headers: {
          "x-upload-token": token,
          "content-type": "application/json",
        },
        body: JSON.stringify({ etags }),
        signal,
      });
    } else {
      await api(`/api/uploads/${id}/complete`, {
        method: "POST",
        headers: { "x-upload-token": token },
        body: null,
        signal,
      });
    }
    const fullUrl = shareUrl(id) + `#key=${keyStr}`;
    return { id, url: fullUrl, keyStr };
  } catch (e) {
    // best-effort abort; ignore failures
    try {
      await api(`/api/uploads/${id}`, {
        method: "DELETE",
        headers: { "x-upload-token": token },
      });
    } catch {}
    throw e;
  }
}
