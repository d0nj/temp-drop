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

const MAX_NAME_BYTES = 512;

function padNameBytes(name) {
  const bytes = new TextEncoder().encode(name);
  if (bytes.length > MAX_NAME_BYTES) {
    // truncate at a UTF-8 boundary: drop partial trailing multi-byte sequence
    let end = MAX_NAME_BYTES;
    while (end > 0 && (bytes[end] & 0xc0) === 0x80) end--;
    return bytes.slice(0, end);
  }
  const padded = new Uint8Array(MAX_NAME_BYTES);
  padded.set(bytes);
  return padded; // zero-padded
}

export async function encryptName(name, key) {
  const encrypted = await encryptChunk(padNameBytes(name), key);
  return Array.from(encrypted).map((b) => b.toString(16).padStart(2, "0")).join("");
}

export async function decryptName(hexStr, key) {
  const bytes = new Uint8Array(hexStr.match(/.{1,2}/g).map((byte) => parseInt(byte, 16)));
  const decrypted = await decryptChunk(bytes, key);
  const padded = new Uint8Array(decrypted);
  let end = padded.length;
  while (end > 0 && padded[end - 1] === 0) end--;
  return new TextDecoder().decode(padded.slice(0, end));
}
