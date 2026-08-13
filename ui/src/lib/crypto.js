// Web Crypto API helper for AES-GCM-256 End-to-End Encryption

export async function generateKey() {
  return await window.crypto.subtle.generateKey(
    { name: "AES-GCM", length: 256 },
    true,
    ["encrypt", "decrypt"]
  );
}

export async function exportKeyToString(key) {
  const exported = await window.crypto.subtle.exportKey("raw", key);
  const bytes = new Uint8Array(exported);
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

export async function importKeyFromString(hexStr) {
  const bytes = new Uint8Array(
    hexStr.match(/.{1,2}/g).map((byte) => parseInt(byte, 16))
  );
  return await window.crypto.subtle.importKey(
    "raw",
    bytes.buffer,
    { name: "AES-GCM" },
    true,
    ["encrypt", "decrypt"]
  );
}

/**
  Encrypts an ArrayBuffer or Uint8Array chunk.
  Format: [12-byte IV][AES-GCM Ciphertext + 16-byte Tag]
*/
export async function encryptChunk(chunkBuffer, key) {
  const iv = window.crypto.getRandomValues(new Uint8Array(12));
  const ciphertext = await window.crypto.subtle.encrypt(
    { name: "AES-GCM", iv },
    key,
    chunkBuffer
  );

  const result = new Uint8Array(iv.byteLength + ciphertext.byteLength);
  result.set(iv, 0);
  result.set(new Uint8Array(ciphertext), iv.byteLength);
  return result;
}

/**
  Decrypts a Uint8Array containing [12-byte IV][Ciphertext + Tag].
*/
export async function decryptChunk(encryptedBytes, key) {
  const iv = encryptedBytes.slice(0, 12);
  const ciphertext = encryptedBytes.slice(12);
  const decrypted = await window.crypto.subtle.decrypt(
    { name: "AES-GCM", iv },
    key,
    ciphertext
  );
  return decrypted;
}

/**
  Encrypts a text string (e.g. filename) to a hex ciphertext string.
*/
export async function encryptText(text, key) {
  const encoder = new TextEncoder();
  const data = encoder.encode(text);
  const encrypted = await encryptChunk(data, key);
  return Array.from(encrypted)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

/**
  Decrypts a hex ciphertext string back to a text string.
*/
export async function decryptText(hexStr, key) {
  const bytes = new Uint8Array(
    hexStr.match(/.{1,2}/g).map((byte) => parseInt(byte, 16))
  );
  const decryptedBuffer = await decryptChunk(bytes, key);
  const decoder = new TextDecoder();
  return decoder.decode(decryptedBuffer);
}
