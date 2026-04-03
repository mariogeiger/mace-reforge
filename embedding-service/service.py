"""
vec2text embedding service.

Endpoints:
  POST /embed     { "texts": ["..."] }              → { "embeddings": [[...], ...] }
  POST /invert    { "embeddings": [[...]], "num_steps": 20 } → { "texts": ["..."] }
  POST /tokenize  { "text": "..." }                 → { "num_tokens": 42 }
  GET  /health                                       → { "status": "ok", "models_loaded": bool }

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

logger = logging.getLogger("vec2text-service")
logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")

# ---------------------------------------------------------------------------
# Model holder with lazy loading and idle unloading
# ---------------------------------------------------------------------------


class ModelHolder:
    def __init__(self):
        self._lock = threading.Lock()
        self._encoder = None
        self._tokenizer = None
        self._corrector = None
        self._last_used: float = 0.0

    @property
    def loaded(self) -> bool:
        return self._encoder is not None

    def _load(self):
        import vec2text
        from transformers import AutoModel, AutoTokenizer

        logger.info("Loading models into GPU memory...")
        t0 = time.monotonic()
        self._corrector = vec2text.load_pretrained_corrector("gtr-base")
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
        self._corrector = None
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
        import vec2text

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
            return vec2text.models.model_utils.mean_pool(
                out.last_hidden_state, inputs["attention_mask"]
            )

    def invert(self, embeddings: torch.Tensor, num_steps: int) -> list[str]:
        self.ensure_loaded()
        import vec2text

        return vec2text.invert_embeddings(
            embeddings=embeddings.cuda(),
            corrector=self._corrector,
            num_steps=num_steps,
        )

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
        _shutdown.wait(60)  # check every minute
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


app = FastAPI(title="vec2text", lifespan=lifespan)


# -- Request/Response models ------------------------------------------------


class EmbedRequest(BaseModel):
    texts: list[str]


class EmbedResponse(BaseModel):
    embeddings: list[list[float]]


class InvertRequest(BaseModel):
    embeddings: list[list[float]]
    num_steps: int = 20


class InvertResponse(BaseModel):
    texts: list[str]


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


@app.post("/invert", response_model=InvertResponse)
async def invert(req: InvertRequest):
    if not req.embeddings:
        raise HTTPException(400, "embeddings must be non-empty")
    tensor = torch.tensor(req.embeddings, dtype=torch.float32)
    texts = models.invert(tensor, num_steps=req.num_steps)
    return InvertResponse(texts=[t.strip() for t in texts])


@app.post("/tokenize", response_model=TokenizeResponse)
async def tokenize(req: TokenizeRequest):
    count = models.tokenize_count(req.text)
    return TokenizeResponse(num_tokens=count)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    uvicorn.run(app, host="127.0.0.1", port=PORT)
