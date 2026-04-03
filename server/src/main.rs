use actix_files::Files;
use actix_web::{App, HttpResponse, HttpServer, middleware, web};
use mace_reforge_shared::{
    AddAnswer, AddOpenAnswer, CreateQuestion, CreateTopic, PlanePoint, PlanePositions, Question,
    Topic, TopicWithCount, User,
};
use std::collections::HashMap;
use std::net::TcpListener;
use std::os::unix::io::FromRawFd;
use std::sync::Mutex;

// ── Database ────────────────────────────────────────────────────────

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Db {
    topics: Vec<Topic>,
    questions: Vec<Question>,
    #[serde(default)]
    users: Vec<User>,
    next_id: u64,
}

impl Db {
    fn new_id(&mut self) -> String {
        self.next_id += 1;
        format!("{}", self.next_id)
    }

    fn question_count(&self, topic_id: &str) -> usize {
        self.questions
            .iter()
            .filter(|q| q.topic_id == topic_id)
            .count()
    }

    fn topic_with_count(&self, t: &Topic) -> TopicWithCount {
        TopicWithCount {
            id: t.id.clone(),
            title: t.title.clone(),
            question_count: self.question_count(&t.id),
        }
    }
}

// ── App state ───────────────────────────────────────────────────────

struct AppState {
    db: Mutex<Db>,
    /// In-memory embedding cache: (question_id, user_name) → embedding vector
    embeddings: Mutex<HashMap<(String, String), Vec<f64>>>,
}

impl AppState {
    fn with_db<R>(&self, f: impl FnOnce(&mut Db) -> R) -> R {
        f(&mut self.db.lock().unwrap())
    }

    fn with_db_save<R>(&self, f: impl FnOnce(&mut Db) -> R) -> R {
        let mut db = self.db.lock().unwrap();
        let result = f(&mut db);
        save_db(&db);
        result
    }
}

// ── Topics ──────────────────────────────────────────────────────────

async fn get_topics(state: web::Data<AppState>) -> HttpResponse {
    state.with_db(|db| {
        let topics: Vec<TopicWithCount> =
            db.topics.iter().map(|t| db.topic_with_count(t)).collect();
        HttpResponse::Ok().json(topics)
    })
}

async fn get_topic(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let topic_id = path.into_inner();
    state.with_db(|db| match db.topics.iter().find(|t| t.id == topic_id) {
        Some(t) => HttpResponse::Ok().json(db.topic_with_count(t)),
        None => HttpResponse::NotFound().finish(),
    })
}

async fn create_topic(state: web::Data<AppState>, body: web::Json<CreateTopic>) -> HttpResponse {
    state.with_db_save(|db| {
        let topic = Topic {
            id: db.new_id(),
            title: body.title.clone(),
        };
        let resp = db.topic_with_count(&topic);
        db.topics.push(topic);
        HttpResponse::Ok().json(resp)
    })
}

// ── Questions ───────────────────────────────────────────────────────

async fn get_questions(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let topic_id = path.into_inner();
    state.with_db(|db| {
        let filtered: Vec<&Question> = db
            .questions
            .iter()
            .filter(|q| q.topic_id == topic_id)
            .collect();
        HttpResponse::Ok().json(filtered)
    })
}

async fn get_question(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (_topic_id, question_id) = path.into_inner();
    state.with_db(|db| match db.questions.iter().find(|q| q.id == question_id) {
        Some(q) => HttpResponse::Ok().json(q),
        None => HttpResponse::NotFound().finish(),
    })
}

async fn create_question(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<CreateQuestion>,
) -> HttpResponse {
    let topic_id = path.into_inner();
    state.with_db_save(|db| {
        let question = Question {
            id: db.new_id(),
            topic_id,
            text: body.text.clone(),
            kind: body.kind.clone(),
            answers: vec![],
            open_answers: vec![],
        };
        let resp = HttpResponse::Ok().json(&question);
        db.questions.push(question);
        resp
    })
}

// ── Answers (closed) ────────────────────────────────────────────────

async fn add_answer(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
    body: web::Json<AddAnswer>,
) -> HttpResponse {
    let (_topic_id, question_id) = path.into_inner();
    state.with_db_save(|db| {
        let Some(q) = db.questions.iter_mut().find(|q| q.id == question_id) else {
            return HttpResponse::NotFound().finish();
        };
        let index = body.index.min(q.answers.len());
        q.answers.insert(index, body.text.clone());
        HttpResponse::Ok().json(q.clone())
    })
}

// ── Answers (open) ──────────────────────────────────────────────────

async fn add_open_answer(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
    body: web::Json<AddOpenAnswer>,
) -> HttpResponse {
    let (_topic_id, question_id) = path.into_inner();

    // Save text to db
    let q = state.with_db_save(|db| {
        let Some(q) = db.questions.iter_mut().find(|q| q.id == question_id) else {
            return None;
        };
        if let Some(existing) = q
            .open_answers
            .iter_mut()
            .find(|a| a.user_name == body.user_name)
        {
            existing.text = body.text.clone();
        } else {
            q.open_answers.push(mace_reforge_shared::OpenAnswer {
                user_name: body.user_name.clone(),
                text: body.text.clone(),
            });
        }
        Some(q.clone())
    });

    let Some(q) = q else {
        return HttpResponse::NotFound().finish();
    };

    // Embed in background (don't block the response)
    let qid = q.id.clone();
    let user_name = body.user_name.clone();
    let text = body.text.clone();
    let state2 = state.clone();
    actix_web::rt::spawn(async move {
        if let Some(emb) = call_embed(&text).await {
            state2
                .embeddings
                .lock()
                .unwrap()
                .insert((qid, user_name), emb);
        }
    });

    HttpResponse::Ok().json(q)
}

// ── Positions (PCA projection) ──────────────────────────────────────

async fn get_positions(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (_topic_id, question_id) = path.into_inner();

    // Collect answers (user_name, text)
    let answers: Vec<(String, String)> = state.with_db(|db| {
        db.questions
            .iter()
            .find(|q| q.id == question_id)
            .map(|q| {
                q.open_answers
                    .iter()
                    .map(|a| (a.user_name.clone(), a.text.clone()))
                    .collect()
            })
            .unwrap_or_default()
    });

    // Embed any answers missing from cache
    let missing: Vec<(String, String)> = {
        let cache = state.embeddings.lock().unwrap();
        answers
            .iter()
            .filter(|(name, _)| !cache.contains_key(&(question_id.clone(), name.clone())))
            .cloned()
            .collect()
    };

    if !missing.is_empty() {
        let texts: Vec<String> = missing.iter().map(|(_, t)| t.clone()).collect();
        if let Some(embs) = call_embed_batch(&texts).await {
            let mut cache = state.embeddings.lock().unwrap();
            for ((name, _), emb) in missing.iter().zip(embs) {
                cache.insert((question_id.clone(), name.clone()), emb);
            }
        }
    }

    let cache = state.embeddings.lock().unwrap();
    let user_names: Vec<String> = answers.iter().map(|(n, _)| n.clone()).collect();
    let embeddings: Vec<Option<&Vec<f64>>> = user_names
        .iter()
        .map(|name| cache.get(&(question_id.clone(), name.clone())))
        .collect();

    let positions = pca_project(&user_names, &embeddings);
    HttpResponse::Ok().json(positions)
}

// ── Embedding service client ────────────────────────────────────────

const EMBEDDING_SERVICE: &str = "http://127.0.0.1:4850";

async fn call_embed(text: &str) -> Option<Vec<f64>> {
    #[derive(serde::Serialize)]
    struct Req {
        texts: Vec<String>,
    }
    #[derive(serde::Deserialize)]
    struct Resp {
        embeddings: Vec<Vec<f64>>,
    }

    let client = awc::Client::default();
    let mut resp = client
        .post(format!("{EMBEDDING_SERVICE}/embed"))
        .insert_header(("Content-Type", "application/json"))
        .send_json(&Req {
            texts: vec![text.to_string()],
        })
        .await
        .ok()?;

    let body = resp.body().limit(1_000_000).await.ok()?;
    let parsed: Resp = serde_json::from_slice(&body).ok()?;
    parsed.embeddings.into_iter().next()
}

async fn call_embed_batch(texts: &[String]) -> Option<Vec<Vec<f64>>> {
    #[derive(serde::Serialize)]
    struct Req {
        texts: Vec<String>,
    }
    #[derive(serde::Deserialize)]
    struct Resp {
        embeddings: Vec<Vec<f64>>,
    }

    let client = awc::Client::default();
    let mut resp = client
        .post(format!("{EMBEDDING_SERVICE}/embed"))
        .insert_header(("Content-Type", "application/json"))
        .send_json(&Req {
            texts: texts.to_vec(),
        })
        .await
        .ok()?;

    let body = resp.body().limit(10_000_000).await.ok()?;
    let parsed: Resp = serde_json::from_slice(&body).ok()?;
    Some(parsed.embeddings)
}

/// Proxy for tokenize (client still needs this for the token counter)
async fn embedding_proxy(
    req: actix_web::HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    let path = req.match_info().get("tail").unwrap_or("");
    let url = format!("{EMBEDDING_SERVICE}/{path}");
    let client = awc::Client::default();
    let fwd = client
        .post(&url)
        .insert_header(("Content-Type", "application/json"));
    match fwd.send_body(body).await {
        Ok(mut resp) => {
            let status = resp.status();
            let body = resp.body().await.unwrap_or_default();
            HttpResponse::build(status)
                .content_type("application/json")
                .body(body)
        }
        Err(e) => {
            log::error!("embedding proxy error: {e}");
            HttpResponse::ServiceUnavailable().body("embedding service unavailable")
        }
    }
}

// ── PCA ─────────────────────────────────────────────────────────────

fn pca_project(user_names: &[String], embeddings: &[Option<&Vec<f64>>]) -> PlanePositions {
    let n = user_names.len();
    if n == 0 {
        return PlanePositions { points: vec![] };
    }

    // Collect valid (index, embedding) pairs
    let valid: Vec<(usize, &Vec<f64>)> = embeddings
        .iter()
        .enumerate()
        .filter_map(|(i, e)| e.map(|v| (i, v)))
        .filter(|(_, v)| !v.is_empty())
        .collect();

    if valid.len() < 2 {
        return PlanePositions {
            points: user_names
                .iter()
                .map(|name| PlanePoint {
                    user_name: name.clone(),
                    x: 0.5,
                    y: 0.5,
                })
                .collect(),
        };
    }

    let dim = valid[0].1.len();

    // Mean
    let mut mean = vec![0.0; dim];
    let nv = valid.len() as f64;
    for (_, e) in &valid {
        for (j, v) in e.iter().enumerate() {
            mean[j] += v / nv;
        }
    }

    // Center
    let centered: Vec<Vec<f64>> = valid
        .iter()
        .map(|(_, e)| e.iter().zip(&mean).map(|(v, m)| v - m).collect())
        .collect();

    // Power iteration for top 2 eigenvectors of X^T X
    let mut axes = Vec::new();
    let mut deflated = centered.clone();

    for _ in 0..2 {
        let mut v = deflated[0].clone();
        let norm: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm < 1e-12 {
            break;
        }
        for x in &mut v {
            *x /= norm;
        }

        for _ in 0..50 {
            let mut w = vec![0.0; dim];
            for row in &deflated {
                let dot: f64 = row.iter().zip(&v).map(|(a, b)| a * b).sum();
                for (j, val) in row.iter().enumerate() {
                    w[j] += dot * val;
                }
            }
            let norm: f64 = w.iter().map(|x| x * x).sum::<f64>().sqrt();
            if norm < 1e-12 {
                break;
            }
            for x in &mut w {
                *x /= norm;
            }
            v = w;
        }

        for row in &mut deflated {
            let dot: f64 = row.iter().zip(&v).map(|(a, b)| a * b).sum();
            for (j, val) in row.iter_mut().enumerate() {
                *val -= dot * v[j];
            }
        }
        axes.push(v);
    }

    if axes.len() < 2 {
        return PlanePositions {
            points: user_names
                .iter()
                .map(|name| PlanePoint {
                    user_name: name.clone(),
                    x: 0.5,
                    y: 0.5,
                })
                .collect(),
        };
    }

    // Project
    let margin = 0.1;
    let range = 1.0 - 2.0 * margin;

    let mut proj: Vec<(f64, f64)> = Vec::new();
    for (_, e) in &valid {
        let cx: Vec<f64> = e.iter().zip(&mean).map(|(v, m)| v - m).collect();
        let px: f64 = cx.iter().zip(&axes[0]).map(|(a, b)| a * b).sum();
        let py: f64 = cx.iter().zip(&axes[1]).map(|(a, b)| a * b).sum();
        proj.push((px, py));
    }

    let min_x = proj.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let max_x = proj.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    let min_y = proj.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let max_y = proj.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    let span_x = (max_x - min_x).max(1e-12);
    let span_y = (max_y - min_y).max(1e-12);

    let mut result: Vec<PlanePoint> = user_names
        .iter()
        .map(|name| PlanePoint {
            user_name: name.clone(),
            x: 0.5,
            y: 0.5,
        })
        .collect();

    for (k, (orig_i, _)) in valid.iter().enumerate() {
        result[*orig_i].x = margin + (proj[k].0 - min_x) / span_x * range;
        result[*orig_i].y = margin + (proj[k].1 - min_y) / span_y * range;
    }

    PlanePositions { points: result }
}

// ── Users ───────────────────────────────────────────────────────────

async fn get_users(state: web::Data<AppState>) -> HttpResponse {
    state.with_db(|db| HttpResponse::Ok().json(&db.users))
}

async fn upsert_user(state: web::Data<AppState>, body: web::Json<User>) -> HttpResponse {
    state.with_db_save(|db| {
        if let Some(existing) = db.users.iter_mut().find(|u| u.name == body.name) {
            existing.shape = body.shape.clone();
            existing.color = body.color.clone();
        } else {
            db.users.push(body.into_inner());
        }
        HttpResponse::Ok().json(&db.users)
    })
}

// ── Health / persistence ────────────────────────────────────────────

async fn health() -> HttpResponse {
    HttpResponse::Ok().body("ok")
}

fn db_path() -> std::path::PathBuf {
    std::path::PathBuf::from("db.json")
}

fn load_db() -> Db {
    std::fs::read_to_string(db_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_db(db: &Db) {
    let json = serde_json::to_string_pretty(db).unwrap();
    let tmp = db_path().with_extension("tmp");
    std::fs::write(&tmp, &json).ok();
    std::fs::rename(&tmp, db_path()).ok();
    backup_db();
}

const BACKUP_DIR: &str = "backups";

fn backup_db() {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let (year, month, day, hour) = unix_secs_to_ymdh(secs);
    let name = format!("{BACKUP_DIR}/db_{year:04}{month:02}{day:02}_{hour:02}.json");

    if std::path::Path::new(&name).exists() {
        return;
    }

    std::fs::create_dir_all(BACKUP_DIR).ok();
    if std::fs::copy(db_path(), &name).is_ok() {
        log::info!("backup: {name}");
    }
}

fn unix_secs_to_ymdh(secs: u64) -> (u64, u64, u64, u64) {
    let hour = (secs / 3600) % 24;
    let days = secs / 86400;
    let mut y = 1970;
    let mut remaining = days;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut m = 0;
    while m < 12 && remaining >= month_days[m] {
        remaining -= month_days[m];
        m += 1;
    }
    (y, (m + 1) as u64, (remaining + 1) as u64, hour)
}

// ── Main ────────────────────────────────────────────────────────────

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let state = web::Data::new(AppState {
        db: Mutex::new(load_db()),
        embeddings: Mutex::new(HashMap::new()),
    });

    log::info!("Server running at http://localhost:4849");

    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(middleware::Logger::new("%a \"%r\" %s %b %Dms"))
            .route("/health", web::get().to(health))
            // Topics
            .route("/api/topics", web::get().to(get_topics))
            .route("/api/topics", web::post().to(create_topic))
            .route("/api/topics/{id}", web::get().to(get_topic))
            // Questions
            .route("/api/topics/{id}/questions", web::get().to(get_questions))
            .route(
                "/api/topics/{id}/questions",
                web::post().to(create_question),
            )
            .route(
                "/api/topics/{topic_id}/questions/{question_id}",
                web::get().to(get_question),
            )
            // Answers (closed)
            .route(
                "/api/topics/{topic_id}/questions/{question_id}/answers",
                web::post().to(add_answer),
            )
            // Answers (open)
            .route(
                "/api/topics/{topic_id}/questions/{question_id}/open-answers",
                web::post().to(add_open_answer),
            )
            // Positions (PCA projection)
            .route(
                "/api/topics/{topic_id}/questions/{question_id}/positions",
                web::get().to(get_positions),
            )
            // Users
            .route("/api/users", web::get().to(get_users))
            .route("/api/users", web::post().to(upsert_user))
            // Embedding proxy (for tokenize only)
            .route("/embedding/{tail:.*}", web::post().to(embedding_proxy))
            // Static files
            .service(Files::new("/", "./client/dist").index_file("index.html"))
    })
    .shutdown_timeout(1);

    let server = if std::env::var("LISTEN_FDS")
        .map(|v| v.parse::<u32>().unwrap_or(0))
        .unwrap_or(0)
        >= 1
    {
        let listener = unsafe { TcpListener::from_raw_fd(3) };
        server.listen(listener)?
    } else {
        server.bind("0.0.0.0:4849")?
    };

    server.run().await
}
