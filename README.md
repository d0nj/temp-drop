# Tempdrop

Single-binary, self-hosted ephemeral file sharing service. Fast, zero-knowledge encrypted, auto-expiring file uploads with zero accounts and zero external database dependencies.

---

## Features

- **Total Zero-Knowledge Encryption**: In-browser AES-256-GCM encryption for both file payload bytes and original filenames. Decryption keys stay strictly in the URL fragment hash (`#key=...`) and are never transmitted to or logged by the server.
- **Single Executable**: Self-contained Rust binary (Axum + SQLite WAL) with the Svelte 5 frontend compiled directly into the executable via `rust-embed`.
- **Real-Time XHR Upload Progress**: Smooth byte-level progress tracking via `XMLHttpRequest` upload events instead of blocky chunk jumps.
- **Chunked Stream Decryption**: Memory-safe 32 MiB sliding window stream decryption for multi-gigabyte files.
- **SVG QR Code Sharing**: Built-in SVG QR code generator on upload completion for mobile share scanning.
- **UTC Timestamp Privacy**: UTC timestamp formatting (`YYYY-MM-DD HH:MM:SS UTC`) to eliminate local browser timezone leakage.
- **Cloudflare & Reverse Proxy Support**: Direct `CF-Connecting-IP` header support and `X-Forwarded-For` handling when `trust_proxy = true`.
- **Expiration Controls**: Time-to-live (TTL) duration choices and max download count limits.
- **Storage Backends**: Local disk storage or S3-compatible object storage (AWS S3, Cloudflare R2, MinIO, RustFS) with presigned multipart uploads.
- **Automated Janitor**: Background task periodically hard-purges expired files, count-exhausted files, and abandoned upload chunks.
- **Protections & Headers**: Fixed-window rate limiting, disk space guards, `Content-Security-Policy`, `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, and `Referrer-Policy: no-referrer`.

---

## Quickstart

### Pre-built Binaries

Single-file executables for Linux, Windows, and macOS are published on [GitHub Releases](https://github.com/d0nj/tempdrop/releases):

- **Linux**: `x86_64` (amd64), `aarch64` (arm64)
- **Windows**: `x86_64` (amd64), `aarch64` (arm64)
- **macOS**: `x86_64` (Intel), `aarch64` (Apple Silicon)

```bash
# Run binary with default configuration or path flag
./tempdrop -c /path/to/tempdrop.toml
```

### Building from Source

```bash
# 1. Build Svelte UI Bundle
cd ui
npm ci
npm run build
cd ..

# 2. Build Release Server Binary
cd server
cargo build --release
```

The compiled binary will be placed at `server/target/release/tempdrop`.

### Running with Docker

```bash
# Build multi-stage Docker image
docker build -t tempdrop:latest .

# Run container
docker run -d -p 8080:8080 -v ./data:/app/data tempdrop:latest
```

---

## Configuration Reference

Configure Tempdrop via `-c /path/to/tempdrop.toml` or environment variables prefixed with `FILEHOST_`.

| Config Key | Env Variable | Default | Description |
|---|---|---|---|
| `server.bind` | `FILEHOST_SERVER_BIND` | `127.0.0.1` | IP address to bind |
| `server.port` | `FILEHOST_SERVER_PORT` | `8080` | Network port |
| `server.trust_proxy` | `FILEHOST_SERVER_TRUST_PROXY` | `false` | Read `CF-Connecting-IP` / `X-Forwarded-For` headers |
| `storage.backend` | `FILEHOST_STORAGE_BACKEND` | `local` | Storage backend (`local` or `s3`) |
| `storage.root_dir` | `FILEHOST_STORAGE_ROOT_DIR` | `./data` | Local storage directory |
| `storage.data_dir` | `FILEHOST_STORAGE_DATA_DIR` | `./data` | Database directory (when storage is `s3`) |
| `uploads.max_size_bytes` | `FILEHOST_UPLOADS_MAX_SIZE_BYTES` | `0` | Max file size in bytes (`0` = unlimited) |
| `uploads.min_free_bytes` | `FILEHOST_UPLOADS_MIN_FREE_BYTES` | `1073741824` | Minimum free disk space in bytes (1 GiB) |
| `uploads.max_ttl_seconds` | `FILEHOST_UPLOADS_MAX_TTL_SECONDS` | `604800` | Max TTL allowed (7 days) |
| `uploads.max_downloads` | `FILEHOST_UPLOADS_MAX_DOWNLOADS` | `100` | Max download limit allowed |
| `uploads.chunk_size_bytes` | `FILEHOST_UPLOADS_CHUNK_SIZE_BYTES` | `33554432` | Part chunk size in bytes (32 MiB) |
| `rate_limit.per_min` | `FILEHOST_RATE_LIMIT_PER_MIN` | `60` | Requests per IP per minute |
| `janitor.interval_seconds` | `FILEHOST_JANITOR_INTERVAL_SECONDS` | `60` | Sweep interval in seconds |

---

## Storage Backends

### Local Storage

```toml
[storage]
backend = "local"
root_dir = "./data"
```

### S3 / Cloudflare R2 / RustFS / MinIO

```toml
[storage]
backend = "s3"

[storage.s3]
bucket = "my-tempdrop-bucket"
region = "us-east-1"
access_key = "YOUR_ACCESS_KEY"
secret_key = "YOUR_SECRET_KEY"
endpoint = "http://127.0.0.1:9000"
force_path_style = true
presign_ttl_seconds = 900
```

---

## Security & Privacy Model

- **Payload & Filename AES-256-GCM**: Client-side Web Crypto API encrypts file payload bytes and original filenames (`enc:<ciphertext_hex>`) before upload. Neither server logs, SQLite database, nor S3 object storage see real filenames or contents.
- **URL Hash Key Storage**: The decryption key is appended to the share link fragment (`#key=...`). Browsers never transmit URL fragments in HTTP request headers or server paths.
- **Cryptographic Tokens**: Upload IDs are 12-character Base62 tokens. Upload authorization tokens are 64-character hex secrets.
- **Input Sanitization**: Filenames are sanitized to prevent CRLF injection, path traversal, and header manipulation.
- **Rate Limiting & Proxy Security**: Fixed-window rate limiting per client IP with `CF-Connecting-IP` priority.
- **Security Headers**: Enforces `Content-Security-Policy`, `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, and `Referrer-Policy: no-referrer`.

---

## API Reference

- `POST /api/uploads` — Initialize upload session
- `PUT /api/uploads/{id}/parts/{part}` — Upload part data (Local backend)
- `GET /api/uploads/{id}/parts/{part}/presign` — Get presigned S3 URL (S3 backend)
- `POST /api/uploads/{id}/complete` — Finalize upload session
- `DELETE /api/uploads/{id}` — Abort upload session
- `GET /api/uploads/{id}` — Get upload metadata
- `GET /raw/{id}` — Stream encrypted payload
- `GET /healthz` — Health check endpoint

---

## Testing

```bash
# Run server test suite
cargo test

# Run S3 integration tests against MinIO
TEMPDROP_TEST_S3=1 \
TEMPDROP_S3_ENDPOINT=http://127.0.0.1:9000 \
TEMPDROP_S3_BUCKET=tempdrop-test \
TEMPDROP_S3_ACCESS_KEY=minioadmin \
TEMPDROP_S3_SECRET_KEY=minioadmin \
cargo test --test s3_minio
```

---

## License

MIT
