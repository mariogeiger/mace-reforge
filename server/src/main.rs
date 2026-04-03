use actix_files::Files;
use actix_web::{App, HttpResponse, HttpServer, middleware, web};
use mace_reforge_shared::{AddAnswer, CreateQuestion, CreateTopic, Question, Topic, TopicWithCount};
use std::net::TcpListener;
use std::os::unix::io::FromRawFd;
use std::sync::Mutex;

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Db {
    topics: Vec<Topic>,
    questions: Vec<Question>,
    next_id: u64,
}

impl Db {
    fn new_id(&mut self) -> String {
        self.next_id += 1;
        format!("{}", self.next_id)
    }

    fn question_count(&self, topic_id: &str) -> usize {
        self.questions.iter().filter(|q| q.topic_id == topic_id).count()
    }

    fn topic_with_count(&self, t: &Topic) -> TopicWithCount {
        TopicWithCount {
            id: t.id.clone(),
            title: t.title.clone(),
            question_count: self.question_count(&t.id),
        }
    }
}

struct AppState {
    db: Mutex<Db>,
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

async fn get_topics(state: web::Data<AppState>) -> HttpResponse {
    state.with_db(|db| {
        let topics: Vec<TopicWithCount> = db.topics.iter().map(|t| db.topic_with_count(t)).collect();
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

async fn get_questions(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let topic_id = path.into_inner();
    state.with_db(|db| {
        let filtered: Vec<&Question> =
            db.questions.iter().filter(|q| q.topic_id == topic_id).collect();
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
            answers: vec![],
        };
        let resp = HttpResponse::Ok().json(&question);
        db.questions.push(question);
        resp
    })
}

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
    // Days since 1970-01-01
    let mut y = 1970;
    let mut remaining = days;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
        if remaining < days_in_year { break; }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 0;
    while m < 12 && remaining >= month_days[m] {
        remaining -= month_days[m];
        m += 1;
    }
    (y, (m + 1) as u64, (remaining + 1) as u64, hour)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let state = web::Data::new(AppState {
        db: Mutex::new(load_db()),
    });

    log::info!("Server running at http://localhost:4849");

    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(middleware::Logger::new("%a \"%r\" %s %b %Dms"))
            .route("/health", web::get().to(health))
            .route("/api/topics", web::get().to(get_topics))
            .route("/api/topics", web::post().to(create_topic))
            .route("/api/topics/{id}", web::get().to(get_topic))
            .route("/api/topics/{id}/questions", web::get().to(get_questions))
            .route(
                "/api/topics/{id}/questions",
                web::post().to(create_question),
            )
            .route(
                "/api/topics/{topic_id}/questions/{question_id}",
                web::get().to(get_question),
            )
            .route(
                "/api/topics/{topic_id}/questions/{question_id}/answers",
                web::post().to(add_answer),
            )
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
