"""
Embedding service using GTR-T5-base (768-dim embeddings).

Endpoints:
  POST /embed     { "texts": ["..."] }   → { "embeddings": [[...], ...] }
  POST /tokenize  { "text": "..." }      → { "num_tokens": 42 }
  GET  /health                            → { "status": "ok", "models_loaded": bool }

Models are loaded lazily on first request and unloaded after IDLE_TIMEOUT_S
seconds of inactivity to free GPU memory.
"""

import logging
import threading
import time
from contextlib import asynccontextmanager

import sdnotify
import torch
import uvicorn
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------

IDLE_TIMEOUT_S = 3600  # 1 hour
WATCHDOG_INTERVAL_S = 15
PORT = 4850

logger = logging.getLogger("embedding-service")
logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")

# ---------------------------------------------------------------------------
# Model holder with lazy loading and idle unloading
# ---------------------------------------------------------------------------


def _mean_pool(hidden_state: torch.Tensor, attention_mask: torch.Tensor) -> torch.Tensor:
    """Mean pooling over non-padded tokens."""
    mask = attention_mask.unsqueeze(-1).float()
    return (hidden_state * mask).sum(dim=1) / mask.sum(dim=1).clamp(min=1e-9)


class ModelHolder:
    def __init__(self):
        self._lock = threading.Lock()
        self._encoder = None
        self._tokenizer = None
        self._last_used: float = 0.0

    @property
    def loaded(self) -> bool:
        return self._encoder is not None

    def _load(self):
        from transformers import AutoModel, AutoTokenizer

        logger.info("Loading models into GPU memory...")
        t0 = time.monotonic()
        self._encoder = AutoModel.from_pretrained(
            "sentence-transformers/gtr-t5-base"
        ).encoder.to("cuda")
        self._tokenizer = AutoTokenizer.from_pretrained(
            "sentence-transformers/gtr-t5-base"
        )
        logger.info(f"Models loaded in {time.monotonic() - t0:.1f}s")

    def _unload(self):
        logger.info("Unloading models from GPU memory...")
        self._encoder = None
        self._tokenizer = None
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
        inputs = self._tokenizer(
            texts,
            return_tensors="pt",
            max_length=128,
            truncation=True,
            padding="max_length",
        ).to("cuda")
        with torch.no_grad():
            out = self._encoder(
                input_ids=inputs["input_ids"],
                attention_mask=inputs["attention_mask"],
            )
            return _mean_pool(out.last_hidden_state, inputs["attention_mask"])

    def tokenize_count(self, text: str) -> int:
        self.ensure_loaded()
        return len(self._tokenizer(text, truncation=False)["input_ids"])


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


class HealthResponse(BaseModel):
    status: str
    models_loaded: bool


# -- Endpoints --------------------------------------------------------------


@app.get("/health", response_model=HealthResponse)
async def health():
    return HealthResponse(status="ok", models_loaded=models.loaded)


@app.post("/embed", response_model=EmbedResponse)
async def embed(req: EmbedRequest):
    if not req.texts:
        raise HTTPException(400, "texts must be non-empty")
    embs = models.embed(req.texts)
    return EmbedResponse(embeddings=embs.cpu().tolist())


@app.post("/tokenize", response_model=TokenizeResponse)
async def tokenize(req: TokenizeRequest):
    count = models.tokenize_count(req.text)
    return TokenizeResponse(num_tokens=count)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    uvicorn.run(app, host="127.0.0.1", port=PORT)
