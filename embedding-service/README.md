# Embedding Service

Text embedding service using BGE-M3 (1024-dim embeddings).

Models are loaded lazily on first request and unloaded after 1 hour of inactivity to free GPU memory.

## Setup

```bash
cd embedding-service
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
```

## Authentication

POST endpoints require a Bearer token. Create `api_keys.txt` (one key per line, `#` comments allowed):

```bash
python3 -c "import secrets; print(secrets.token_urlsafe(32))" >> api_keys.txt
```

Localhost requests (127.0.0.1) bypass auth, so the Rust server doesn't need a key.

## API

Port: **4850** / Public: **https://embedding.geiger.ink**

### `GET /health`

```json
{ "status": "ok", "models_loaded": false }
```

### `GET /info`

```json
{ "max_tokens": 8192 }
```

### `POST /embed`

```bash
curl -X POST https://embedding.geiger.ink/embed \
  -H 'Authorization: Bearer <key>' \
  -H 'Content-Type: application/json' \
  -d '{"texts": ["hello world", "another sentence"]}'
```

```json
{ "embeddings": [[0.034345, 0.033161, ...], [0.021913, -0.037134, ...]] }
```

Each embedding is a 1024-dim float vector. Latency: ~8ms per text.

### `POST /tokenize`

```bash
curl -X POST https://embedding.geiger.ink/tokenize \
  -H 'Authorization: Bearer <key>' \
  -H 'Content-Type: application/json' \
  -d '{"text": "hello world"}'
```

```json
{ "num_tokens": 3 }
```

The embedding model truncates at 8192 tokens.

## Systemd

Copy the unit file:

```bash
cp embedding.service ~/.config/systemd/user/
```

Edit the paths in `embedding.service` if your repo is not at `/home/mario/git/mace-reforge`.

Then:

```bash
systemctl --user daemon-reload
systemctl --user enable --now embedding.service
```

The service includes a watchdog (30s timeout) and auto-restarts on failure.

```bash
systemctl --user status embedding       # check status
systemctl --user restart embedding      # restart
journalctl --user -u embedding -f       # follow logs
```
