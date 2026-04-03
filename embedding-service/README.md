# Embedding Service

Text embedding and inversion service using [vec2text](https://github.com/vec2text/vec2text) with GTR-T5-base (768-dim embeddings).

Models are loaded lazily on first request and unloaded after 1 hour of inactivity to free GPU memory.

## Setup

```bash
cd embedding-service
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
```

## API

Port: **4850**

### `GET /health`

```json
{ "status": "ok", "models_loaded": false }
```

### `POST /embed`

```bash
curl -X POST http://127.0.0.1:4850/embed \
  -H 'Content-Type: application/json' \
  -d '{"texts": ["hello world", "another sentence"]}'
```

```json
{ "embeddings": [[0.15, 0.09, ...], [0.06, 0.07, ...]] }
```

Each embedding is a 768-dim float vector. Latency: ~5ms per text.

### `POST /invert`

```bash
curl -X POST http://127.0.0.1:4850/invert \
  -H 'Content-Type: application/json' \
  -d '{"embeddings": [[0.15, 0.09, ...]], "num_steps": 20}'
```

```json
{ "texts": ["hello world"] }
```

`num_steps` controls quality vs speed (default 20, ~3s). Use 5 for faster results (~800ms).

### `POST /tokenize`

```bash
curl -X POST http://127.0.0.1:4850/tokenize \
  -H 'Content-Type: application/json' \
  -d '{"text": "hello world"}'
```

```json
{ "num_tokens": 3 }
```

The embedding model truncates at 128 tokens (~80 words).

## Systemd

Copy the unit file:

```bash
cp vec2text.service ~/.config/systemd/user/
```

Edit the paths in `vec2text.service` if your repo is not at `/home/mario/git/mace-reforge`.

Then:

```bash
systemctl --user daemon-reload
systemctl --user enable --now vec2text.service
```

The service includes a watchdog (30s timeout) and auto-restarts on failure.

```bash
systemctl --user status vec2text       # check status
systemctl --user restart vec2text      # restart
journalctl --user -u vec2text -f       # follow logs
```
