import { importKeyFromString, decryptChunk } from "./crypto.js";

export async function downloadAndDecryptFile({ id, filename, keyStr, chunkSize, onProgress }) {
  const key = await importKeyFromString(keyStr);
  const response = await fetch(`/raw/${id}`);
  if (!response.ok) {
    throw new Error(`Download failed with status ${response.status}`);
  }

  const reader = response.body.getReader();
  const contentLength = parseInt(response.headers.get("content-length") || "0", 10);
  const ENCRYPTED_CHUNK_SIZE = chunkSize + 28; // 12-byte IV + 16-byte Tag

  let receivedBytes = 0;
  let buffer = new Uint8Array(0);
  const decryptedBlobs = [];

  function appendToBuffer(chunk) {
    const nextBuffer = new Uint8Array(buffer.length + chunk.length);
    nextBuffer.set(buffer, 0);
    nextBuffer.set(chunk, buffer.length);
    buffer = nextBuffer;
  }

  while (true) {
    const { done, value } = await reader.read();
    if (value) {
      appendToBuffer(value);
      receivedBytes += value.length;
      if (onProgress && contentLength > 0) {
        onProgress(receivedBytes, contentLength);
      }
    }

    // Decrypt all complete encrypted 32MiB chunks
    while (buffer.length >= ENCRYPTED_CHUNK_SIZE) {
      const encryptedChunk = buffer.slice(0, ENCRYPTED_CHUNK_SIZE);
      buffer = buffer.slice(ENCRYPTED_CHUNK_SIZE);

      const decrypted = await decryptChunk(encryptedChunk, key);
      decryptedBlobs.push(decrypted);
    }

    if (done) break;
  }

  // Decrypt remaining final chunk
  if (buffer.length > 0) {
    if (buffer.length < 28) {
      throw new Error("Corrupted ciphertext payload: truncated chunk");
    }
    const decrypted = await decryptChunk(buffer, key);
    decryptedBlobs.push(decrypted);
    buffer = new Uint8Array(0);
  }

  // Construct final blob and trigger file download
  const blob = new Blob(decryptedBlobs, { type: "application/octet-stream" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename || "decrypted-file";
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}
