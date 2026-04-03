# MACE-REFORGE

## Tech Stack

Rust workspace (shared/server/client). Leptos 0.8 CSR WASM frontend, Actix-web 4 backend, file-based JSON persistence, Trunk bundler. Systemd socket activation + watchdog. See `48hfp` project for the identical stack.

## Visual Design

- **Palette**: warm cream `#EDE8DB` background, rich brown `#80572B` accents
- **Motif**: four-pointed diamond star (SVG path), used in the header logo and as decorative elements
- **Typography**: Georgia/serif for headings (light weight), system sans-serif for body
- **Cards**: translucent white (`rgba(255,255,255,0.5)`) with subtle brown borders, hover lifts

## Text Style

All user-facing messages on the voting circle are written in the style of **Ben Jonson** — elevated but clear, classical but direct. Not flowery; precise. Think "Drink to me only with thine eyes."

Examples:
- Empty circle: *"The circle stands empty, a stage awaiting its players. Pray, touch it, and set forth a position."*
- One answer: *"A solitary voice echoes — yet true discourse demands a partner."*
- No opinion: *"I remain unswayed, holding no fixed position in this matter."*
- Strong agreement: *"With settled conviction, I hold firmly with [answer]."*

Form placeholders and button labels can be lightly Jonsonian but stay functional.

## Code Style

Maximize **power/complexity ratio** — use cases handled per branch in the code.

- **One mutex, one lock scope**: single `Mutex<Db>` with `with_db`/`with_db_save` helpers. Eliminates deadlock classes entirely.
- **Derived state over stored state**: `question_count` is computed on read, never stored or manually synchronized.
- **Data-driven over if-chains**: opinion text thresholds are a `BANDS` table. Adding a level = adding a row, not a branch.
- **Typed API helpers**: `api_get<T>`/`api_post<T>` deserialize directly to target type. No intermediate `Value` juggling.
- **Uniform algorithms**: `answer_angle`, `angular_distance`, `insertion_index` handle 0/1/N answers without special cases.
- **Push decisions to edges**: click angle → insertion index is computed once at the boundary, then the server just does `vec.insert(index, text)`.

## Ports

- Server: 4849
- Trunk dev: 8081

## Build & Deploy

```bash
cargo build --release -p mace-reforge-server && \
trunk build --release --config client/Trunk.toml && \
systemctl --user restart mace-reforge.service
```
