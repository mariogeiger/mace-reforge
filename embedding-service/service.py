"""
Embedding service using BGE-M3 (1024-dim embeddings).

Endpoints:
  POST /embed     { "texts": ["..."] }   → { "embeddings": [[...], ...] }
  POST /tokenize  { "text": "..." }      → { "num_tokens": 42 }
  GET  /info                              → { "max_tokens": 8192 }
  GET  /health                            → { "status": "ok", "models_loaded": bool }

Models are loaded lazily on first request and unloaded after IDLE_TIMEOUT_S
seconds of inactivity to free GPU memory.
"""

import logging
import pathlib
import threading
import time
from contextlib import asynccontextmanager

import sdnotify
import torch
import uvicorn
from fastapi import Depends, FastAPI, HTTPException, Request
from pydantic import BaseModel

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------

MODEL_NAME = "BAAI/bge-m3"
MAX_TOKENS = 8192
IDLE_TIMEOUT_S = 3600  # 1 hour
WATCHDOG_INTERVAL_S = 15
PORT = 4850
API_KEYS_FILE = pathlib.Path(__file__).parent / "api_keys.txt"

logger = logging.getLogger("embedding-service")
logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")

# ---------------------------------------------------------------------------
# Auth
# ---------------------------------------------------------------------------

def _load_api_keys() -> set[str]:
    if not API_KEYS_FILE.exists():
        return set()
    return {line.strip() for line in API_KEYS_FILE.read_text().splitlines() if line.strip() and not line.startswith("#")}


async def require_key(request: Request):
    client = request.client
    if client and client.host == "127.0.0.1":
        return
    auth = request.headers.get("Authorization", "")
    if not auth.startswith("Bearer "):
        raise HTTPException(401, "Missing API key")
    keys = _load_api_keys()
    if not keys or auth[7:] not in keys:
        raise HTTPException(401, "Invalid API key")

# ---------------------------------------------------------------------------
# Model holder with lazy loading and idle unloading
# ---------------------------------------------------------------------------


class ModelHolder:
    def __init__(self):
        self._lock = threading.Lock()
        self._model = None
        self._last_used: float = 0.0

    @property
    def loaded(self) -> bool:
        return self._model is not None

    def _load(self):
        from sentence_transformers import SentenceTransformer

        logger.info(f"Loading {MODEL_NAME} into GPU memory...")
        t0 = time.monotonic()
        self._model = SentenceTransformer(MODEL_NAME, device="cuda")
        logger.info(f"Model loaded in {time.monotonic() - t0:.1f}s")

    def _unload(self):
        logger.info("Unloading model from GPU memory...")
        self._model = None
        torch.cuda.empty_cache()
        logger.info("GPU memory freed")

    def ensure_loaded(self):
        with self._lock:
            if not self.loaded:
                self._load()
            self._last_used = time.monotonic()

    def maybe_unload(self):
        with self._lock:
            if self.loaded and (time.monotonic() - self._last_used > IDLE_TIMEOUT_S):
                self._unload()

    def embed(self, texts: list[str]) -> torch.Tensor:
        self.ensure_loaded()
        return self._model.encode(texts, convert_to_tensor=True)

    def tokenize_count(self, text: str) -> int:
        self.ensure_loaded()
        return len(self._model.tokenizer(text, truncation=False)["input_ids"])


models = ModelHolder()

# ---------------------------------------------------------------------------
# Background threads
# ---------------------------------------------------------------------------

_shutdown = threading.Event()


def _idle_checker():
    """Periodically check if models should be unloaded."""
    while not _shutdown.is_set():
        _shutdown.wait(60)
        models.maybe_unload()


def _watchdog_pinger():
    """Send systemd watchdog pings."""
    n = sdnotify.SystemdNotifier(debug=False)
    n.notify("READY=1")
    while not _shutdown.is_set():
        n.notify("WATCHDOG=1")
        _shutdown.wait(WATCHDOG_INTERVAL_S)


# ---------------------------------------------------------------------------
# FastAPI app
# ---------------------------------------------------------------------------


@asynccontextmanager
async def lifespan(app: FastAPI):
    idle_thread = threading.Thread(target=_idle_checker, daemon=True)
    watchdog_thread = threading.Thread(target=_watchdog_pinger, daemon=True)
    idle_thread.start()
    watchdog_thread.start()
    logger.info(f"Service started on port {PORT}")
    yield
    _shutdown.set()


app = FastAPI(title="embedding-service", lifespan=lifespan)


# -- Request/Response models ------------------------------------------------


class EmbedRequest(BaseModel):
    texts: list[str]


class EmbedResponse(BaseModel):
    embeddings: list[list[float]]


class TokenizeRequest(BaseModel):
    text: str


class TokenizeResponse(BaseModel):
    num_tokens: int


class InfoResponse(BaseModel):
    max_tokens: int


class HealthResponse(BaseModel):
    status: str
    models_loaded: bool


# -- Endpoints --------------------------------------------------------------


@app.get("/health", response_model=HealthResponse)
async def health():
    return HealthResponse(status="ok", models_loaded=models.loaded)


@app.get("/info", response_model=InfoResponse)
async def info():
    return InfoResponse(max_tokens=MAX_TOKENS)


@app.post("/embed", response_model=EmbedResponse, dependencies=[Depends(require_key)])
async def embed(req: EmbedRequest):
    if not req.texts:
        raise HTTPException(400, "texts must be non-empty")
    embs = models.embed(req.texts)
    return EmbedResponse(
        embeddings=[[round(x, 6) for x in row] for row in embs.cpu().tolist()]
    )


@app.post("/tokenize", response_model=TokenizeResponse, dependencies=[Depends(require_key)])
async def tokenize(req: TokenizeRequest):
    count = models.tokenize_count(req.text)
    return TokenizeResponse(num_tokens=count)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    uvicorn.run(app, host="127.0.0.1", port=PORT)
