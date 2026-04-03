use leptos::prelude::*;
use mace_reforge_shared::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

macro_rules! log {
    ($($t:tt)*) => {
        web_sys::console::log_1(&format!($($t)*).into())
    };
}

// ── API helpers ──────────────────────────────────────────────────────

async fn fetch_json(
    url: &str,
    opts: &web_sys::RequestInit,
) -> Result<serde_json::Value, String> {
    let window = web_sys::window().unwrap();
    let resp: web_sys::Response = JsFuture::from(window.fetch_with_str_and_init(url, opts))
        .await
        .map_err(|e| format!("{e:?}"))?
        .dyn_into()
        .map_err(|e| format!("{e:?}"))?;
    let status = resp.status();
    let text = JsFuture::from(resp.text().map_err(|e| format!("{e:?}"))?)
        .await
        .map_err(|e| format!("{e:?}"))?
        .as_string()
        .unwrap_or_default();
    if status >= 400 {
        return Err(format!("HTTP {status}: {text}"));
    }
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

async fn api_get<T: serde::de::DeserializeOwned>(url: &str) -> Result<T, String> {
    let opts = web_sys::RequestInit::new();
    opts.set_method("GET");
    let json = fetch_json(url, &opts).await?;
    serde_json::from_value(json).map_err(|e| e.to_string())
}

async fn api_post<T: serde::de::DeserializeOwned>(
    url: &str,
    body: &impl serde::Serialize,
) -> Result<T, String> {
    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    let headers = web_sys::Headers::new().unwrap();
    headers.set("Content-Type", "application/json").unwrap();
    opts.set_headers(&headers);
    opts.set_body(&JsValue::from_str(&serde_json::to_string(body).unwrap()));
    let json = fetch_json(url, &opts).await?;
    serde_json::from_value(json).map_err(|e| e.to_string())
}

// ── Local storage helpers ───────────────────────────────────────────

fn storage() -> web_sys::Storage {
    web_sys::window()
        .unwrap()
        .local_storage()
        .unwrap()
        .unwrap()
}

fn load_local_user() -> Option<User> {
    let s = storage().get_item("user").ok()??;
    serde_json::from_str(&s).ok()
}

fn save_local_user(user: &User) {
    let json = serde_json::to_string(user).unwrap();
    storage().set_item("user", &json).ok();
}

// ── Shapes: SVG rendering ───────────────────────────────────────────

const ALL_SHAPES: &[Shape] = &[
    Shape::Circle,
    Shape::Square,
    Shape::Triangle,
    Shape::Diamond,
    Shape::Star,
    Shape::Hexagon,
];

const PALETTE: &[&str] = &[
    "#c0392b", "#e67e22", "#f1c40f", "#27ae60", "#2980b9", "#8e44ad", "#e84393", "#2d3436",
];

fn shape_svg(shape: Shape, color: String, size: f64) -> impl IntoView {
    let s = size;
    let half = s / 2.0;
    let vb = format!("0 0 {s} {s}");
    let inner = match &shape {
        Shape::Circle => format!(
            r#"<circle cx="{half}" cy="{half}" r="{}" fill="{color}"/>"#,
            half * 0.85
        ),
        Shape::Square => {
            let inset = s * 0.12;
            let side = s - inset * 2.0;
            format!(
                r#"<rect x="{inset}" y="{inset}" width="{side}" height="{side}" rx="{}" fill="{color}"/>"#,
                s * 0.08
            )
        }
        Shape::Triangle => {
            let top = s * 0.1;
            let bot = s * 0.9;
            format!(
                r#"<polygon points="{half},{top} {bot},{bot} {top},{bot}" fill="{color}"/>"#
            )
        }
        Shape::Diamond => {
            let m = s * 0.08;
            let e = s - m;
            format!(
                r#"<polygon points="{half},{m} {e},{half} {half},{e} {m},{half}" fill="{color}"/>"#
            )
        }
        Shape::Star => {
            let cx = half;
            let cy = half;
            let ro = half * 0.9;
            let ri = half * 0.35;
            let mut pts = String::new();
            for i in 0..10 {
                let angle =
                    std::f64::consts::FRAC_PI_2 * -1.0 + std::f64::consts::PI * i as f64 / 5.0;
                let r = if i % 2 == 0 { ro } else { ri };
                if !pts.is_empty() {
                    pts.push(' ');
                }
                pts.push_str(&format!(
                    "{:.1},{:.1}",
                    cx + r * angle.cos(),
                    cy + r * angle.sin()
                ));
            }
            format!(r#"<polygon points="{pts}" fill="{color}"/>"#)
        }
        Shape::Hexagon => {
            let cx = half;
            let cy = half;
            let r = half * 0.88;
            let mut pts = String::new();
            for i in 0..6 {
                let angle = std::f64::consts::PI / 3.0 * i as f64 - std::f64::consts::FRAC_PI_2;
                if !pts.is_empty() {
                    pts.push(' ');
                }
                pts.push_str(&format!(
                    "{:.1},{:.1}",
                    cx + r * angle.cos(),
                    cy + r * angle.sin()
                ));
            }
            format!(r#"<polygon points="{pts}" fill="{color}"/>"#)
        }
    };

    view! {
        <svg
            viewBox=vb
            xmlns="http://www.w3.org/2000/svg"
            inner_html=inner
        />
    }
}

fn shape_name(shape: &Shape) -> &'static str {
    match shape {
        Shape::Circle => "Circle",
        Shape::Square => "Square",
        Shape::Triangle => "Triangle",
        Shape::Diamond => "Diamond",
        Shape::Star => "Star",
        Shape::Hexagon => "Hexagon",
    }
}

// ── Routing ─────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum Route {
    Home,
    Topic(String),
    Question(String, String),
}

fn parse_hash() -> Route {
    let hash = web_sys::window()
        .unwrap()
        .location()
        .hash()
        .unwrap_or_default();
    let path = hash.trim_start_matches('#');
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match parts.as_slice() {
        ["topic", tid, "question", qid] => Route::Question(tid.to_string(), qid.to_string()),
        ["topic", tid] => Route::Topic(tid.to_string()),
        _ => Route::Home,
    }
}

const STAR_PATH: &str = "M 211.88,0 C 211.88,172.25 173.19,210.94 0.94,210.94 173.19,210.94 211.88,249.63 211.88,421.88 211.88,249.63 250.57,210.94 422.82,210.94 250.57,210.94 211.88,172.25 211.88,0 Z";

#[component]
fn Star(class_name: &'static str) -> impl IntoView {
    view! {
        <div class=class_name>
            <svg viewBox="0 0 423 422" xmlns="http://www.w3.org/2000/svg">
                <path d=STAR_PATH fill="currentColor"/>
            </svg>
        </div>
    }
}

// ── Entry ───────────────────────────────────────────────────────────

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let (route, set_route) = signal(parse_hash());
    let (current_user, set_current_user) = signal(load_local_user());

    // Listen for hashchange
    Effect::new(move || {
        let closure = wasm_bindgen::closure::Closure::<dyn Fn()>::new(move || {
            set_route.set(parse_hash());
        });
        web_sys::window()
            .unwrap()
            .add_event_listener_with_callback("hashchange", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    });

    view! {
        <header class="site-header">
            <a href="#/" class="logo">
                <svg class="star" viewBox="0 0 423 422" xmlns="http://www.w3.org/2000/svg">
                    <path d=STAR_PATH fill="currentColor"/>
                </svg>
                <span class="logo-text">"MACE-REFORGE"</span>
            </a>
            <UserBadge user=current_user set_user=set_current_user/>
        </header>
        <main>
            {move || match route.get() {
                Route::Home => view! { <HomePage/> }.into_any(),
                Route::Topic(id) => view! { <TopicPage topic_id=id/> }.into_any(),
                Route::Question(tid, qid) => view! { <QuestionPage topic_id=tid question_id=qid current_user=current_user/> }.into_any(),
            }}
        </main>
    }
}

// ── User Badge (header) ─────────────────────────────────────────────

#[component]
fn UserBadge(
    user: ReadSignal<Option<User>>,
    set_user: WriteSignal<Option<User>>,
) -> impl IntoView {
    let (editing, set_editing) = signal(false);
    let (name_input, set_name_input) = signal(String::new());
    let (selected_shape, set_selected_shape) = signal(Shape::Circle);
    let (selected_color, set_selected_color) = signal(PALETTE[0].to_string());
    let (known_users, set_known_users) = signal(Vec::<User>::new());
    let (suggestions, set_suggestions) = signal(Vec::<User>::new());

    // Load known users when editing starts
    let load_users = move || {
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(users) = api_get::<Vec<User>>("/api/users").await {
                set_known_users.set(users);
            }
        });
    };

    let open_editor = move |_: web_sys::MouseEvent| {
        if let Some(u) = user.get_untracked() {
            set_name_input.set(u.name);
            set_selected_shape.set(u.shape);
            set_selected_color.set(u.color);
        }
        load_users();
        set_editing.set(true);
    };

    let save_user = move || {
        let name = name_input.get_untracked();
        if name.trim().is_empty() {
            return;
        }
        let new_user = User {
            name: name.trim().to_string(),
            shape: selected_shape.get_untracked(),
            color: selected_color.get_untracked(),
        };
        save_local_user(&new_user);
        set_user.set(Some(new_user.clone()));
        set_editing.set(false);
        wasm_bindgen_futures::spawn_local(async move {
            api_post::<Vec<User>>("/api/users", &new_user).await.ok();
        });
    };

    let on_name_input = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        set_name_input.set(val.clone());
        let val_lower = val.to_lowercase();
        let filtered: Vec<User> = known_users
            .get_untracked()
            .into_iter()
            .filter(|u| !val_lower.is_empty() && u.name.to_lowercase().contains(&val_lower))
            .collect();
        set_suggestions.set(filtered);
    };

    let select_suggestion = move |u: User| {
        set_name_input.set(u.name.clone());
        set_selected_shape.set(u.shape.clone());
        set_selected_color.set(u.color.clone());
        set_suggestions.set(vec![]);
    };

    view! {
        <div class="user-badge-area">
            <Show
                when=move || !editing.get()
                fallback=move || {
                    let save = save_user.clone();
                    view! {
                        <div class="user-editor">
                            <div class="user-editor-name">
                                <input
                                    type="text"
                                    placeholder="Thy name..."
                                    prop:value=name_input
                                    on:input=on_name_input
                                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                                        if ev.key() == "Enter" { save_user(); }
                                        if ev.key() == "Escape" { set_editing.set(false); }
                                    }
                                />
                                <Show when=move || !suggestions.get().is_empty()>
                                    <div class="user-suggestions">
                                        <For
                                            each=move || suggestions.get()
                                            key=|u| u.name.clone()
                                            let:u
                                        >
                                            {
                                                let uc = u.clone();
                                                let uc2 = u.clone();
                                                view! {
                                                    <div class="user-suggestion" on:mousedown=move |_| select_suggestion(uc.clone())>
                                                        <span class="suggestion-avatar">{shape_svg(uc2.shape.clone(), uc2.color.clone(), 20.0)}</span>
                                                        <span>{u.name.clone()}</span>
                                                    </div>
                                                }
                                            }
                                        </For>
                                    </div>
                                </Show>
                            </div>
                            <div class="shape-picker">
                                {ALL_SHAPES.iter().map(|s| {
                                    let for_selected = s.clone();
                                    let for_click = s.clone();
                                    let for_svg = s.clone();
                                    view! {
                                        <button
                                            class="shape-option"
                                            class:selected=move || selected_shape.get() == for_selected
                                            title=shape_name(s)
                                            on:click=move |_| set_selected_shape.set(for_click.clone())
                                        >
                                            {shape_svg(for_svg, selected_color.get(), 24.0)}
                                        </button>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                            <div class="color-picker">
                                {PALETTE.iter().map(|c| {
                                    let c = c.to_string();
                                    let c2 = c.clone();
                                    let c3 = c.clone();
                                    view! {
                                        <button
                                            class="color-option"
                                            class:selected=move || selected_color.get() == c
                                            style:background=c2.clone()
                                            on:click=move |_| set_selected_color.set(c3.clone())
                                        />
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                            <button class="user-save-btn" on:click=move |_| save()>"Enter"</button>
                        </div>
                    }
                }
            >
                {move || {
                    if let Some(u) = user.get() {
                        view! {
                            <button class="user-badge" on:click=open_editor>
                                <span class="badge-avatar">{shape_svg(u.shape.clone(), u.color.clone(), 28.0)}</span>
                                <span class="badge-name">{u.name}</span>
                            </button>
                        }.into_any()
                    } else {
                        view! {
                            <button class="user-badge user-badge-empty" on:click=open_editor>
                                "Set thy identity"
                            </button>
                        }.into_any()
                    }
                }}
            </Show>
        </div>
    }
}

// ── Home Page: Topic Grid ───────────────────────────────────────────

#[component]
fn HomePage() -> impl IntoView {
    let (topics, set_topics) = signal(Vec::<TopicWithCount>::new());
    let (new_title, set_new_title) = signal(String::new());
    let (error, set_error) = signal(Option::<String>::None);

    Effect::new(move || {
        wasm_bindgen_futures::spawn_local(async move {
            match api_get::<Vec<TopicWithCount>>("/api/topics").await {
                Ok(t) => set_topics.set(t),
                Err(e) => {
                    log!("[HomePage] {e}");
                    set_error.set(Some(e));
                }
            }
        });
    });

    let do_create = move || {
        let title = new_title.get_untracked();
        if title.trim().is_empty() {
            return;
        }
        set_new_title.set(String::new());
        wasm_bindgen_futures::spawn_local(async move {
            match api_post::<TopicWithCount>("/api/topics", &CreateTopic { title }).await {
                Ok(topic) => set_topics.update(|t| t.push(topic)),
                Err(e) => {
                    log!("[create_topic] {e}");
                    set_error.set(Some(e));
                }
            }
        });
    };

    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        if ev.key() == "Enter" {
            do_create();
        }
    };

    let on_click = move |_: web_sys::MouseEvent| {
        do_create();
    };

    view! {
        <div class="page home-page">
            <h1>"Discourses"</h1>
            <div class="create-form">
                <input
                    type="text"
                    placeholder="Name thy discourse..."
                    prop:value=new_title
                    on:input=move |ev| set_new_title.set(event_target_value(&ev))
                    on:keydown=on_keydown
                />
                <button on:click=on_click>"Establish"</button>
            </div>
            <Show when=move || error.get().is_some()>
                <p class="error">{move || error.get().unwrap_or_default()}</p>
            </Show>
            <div class="topic-grid">
                <For
                    each=move || topics.get()
                    key=|t| t.id.clone()
                    let:topic
                >
                    <a class="topic-card" href=format!("#/topic/{}", topic.id)>
                        <Star class_name="card-star"/>
                        <span class="card-title">{topic.title}</span>
                        <span class="card-count">{topic.question_count}" questions within"</span>
                    </a>
                </For>
            </div>
        </div>
    }
}

// ── Topic Page: Questions List ──────────────────────────────────────

#[component]
fn TopicPage(topic_id: String) -> impl IntoView {
    let (topic, set_topic) = signal(Option::<TopicWithCount>::None);
    let (questions, set_questions) = signal(Vec::<Question>::new());
    let (new_text, set_new_text) = signal(String::new());
    let (new_kind, set_new_kind) = signal(QuestionKind::Closed);

    let tid = topic_id.clone();
    Effect::new(move || {
        let tid = tid.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(t) = api_get::<TopicWithCount>(&format!("/api/topics/{tid}")).await {
                set_topic.set(Some(t));
            }
            if let Ok(q) = api_get::<Vec<Question>>(&format!("/api/topics/{tid}/questions")).await {
                set_questions.set(q);
            }
        });
    });

    let tid2 = topic_id.clone();
    let do_create = move || {
        let text = new_text.get_untracked();
        if text.trim().is_empty() {
            return;
        }
        set_new_text.set(String::new());
        let tid = tid2.clone();
        let kind = new_kind.get_untracked();
        wasm_bindgen_futures::spawn_local(async move {
            match api_post::<Question>(
                &format!("/api/topics/{tid}/questions"),
                &CreateQuestion { text, kind },
            )
            .await
            {
                Ok(q) => set_questions.update(|qs| qs.push(q)),
                Err(e) => log!("[create_question] {e}"),
            }
        });
    };

    let do_create_k = do_create.clone();
    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        if ev.key() == "Enter" {
            do_create_k();
        }
    };

    let on_click = move |_: web_sys::MouseEvent| {
        do_create();
    };

    let tid3 = topic_id.clone();

    view! {
        <div class="page topic-page">
            <a href="#/" class="back-link">"Return to discourses"</a>
            <h1>{move || topic.get().map(|t| t.title).unwrap_or_default()}</h1>
            <div class="create-form">
                <input
                    type="text"
                    placeholder="What matter shall be put to question?"
                    prop:value=new_text
                    on:input=move |ev| set_new_text.set(event_target_value(&ev))
                    on:keydown=on_keydown
                />
                <div class="kind-toggle">
                    <button
                        class="kind-btn"
                        class:active=move || new_kind.get() == QuestionKind::Closed
                        on:click=move |_| set_new_kind.set(QuestionKind::Closed)
                    >"Closed"</button>
                    <button
                        class="kind-btn"
                        class:active=move || new_kind.get() == QuestionKind::Open
                        on:click=move |_| set_new_kind.set(QuestionKind::Open)
                    >"Open"</button>
                </div>
                <button on:click=on_click>"Propose"</button>
            </div>
            <div class="question-list">
                <For
                    each=move || questions.get()
                    key=|q| q.id.clone()
                    let:question
                >
                    {
                        let tid = tid3.clone();
                        let qid = question.id.clone();
                        let is_open = question.kind == QuestionKind::Open;
                        let n = if is_open { question.open_answers.len() } else { question.answers.len() };
                        let subtitle = if n == 0 {
                            "yet unvoiced".to_string()
                        } else {
                            format!("{n} positions voiced")
                        };
                        let kind_label = if is_open { "open" } else { "closed" };
                        view! {
                            <a class="question-card" href=format!("#/topic/{tid}/question/{qid}")>
                                <span class="question-text">{question.text}</span>
                                <span class="question-meta">
                                    <span class="kind-label">{kind_label}</span>
                                    <span class="answer-count">{subtitle}</span>
                                </span>
                            </a>
                        }
                    }
                </For>
            </div>
        </div>
    }
}

// ── Question Page: dispatch by kind ─────────────────────────────────

#[component]
fn QuestionPage(
    topic_id: String,
    question_id: String,
    current_user: ReadSignal<Option<User>>,
) -> impl IntoView {
    let (question, set_question) = signal(Option::<Question>::None);
    let (kind, set_kind) = signal(Option::<QuestionKind>::None);

    let tid = topic_id.clone();
    let qid = question_id.clone();
    Effect::new(move || {
        let tid = tid.clone();
        let qid = qid.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(q) =
                api_get::<Question>(&format!("/api/topics/{tid}/questions/{qid}")).await
            {
                set_kind.set(Some(q.kind.clone()));
                set_question.set(Some(q));
            }
        });
    });

    let tid2 = topic_id.clone();
    let qid2 = question_id.clone();

    // Dispatch based on `kind` (set once), not `question` (updated on every submit)
    view! {
        {move || {
            let Some(k) = kind.get() else {
                return view! { <div class="page">"Loading..."</div> }.into_any();
            };
            match k {
                QuestionKind::Closed => view! {
                    <ClosedQuestionPage
                        topic_id=tid2.clone()
                        question=question
                        set_question=set_question
                    />
                }.into_any(),
                QuestionKind::Open => view! {
                    <OpenQuestionPage
                        topic_id=tid2.clone()
                        question_id=qid2.clone()
                        question=question
                        set_question=set_question
                        current_user=current_user
                    />
                }.into_any(),
            }
        }}
    }
}

// ── Closed Question: Voting Circle ──────────────────────────────────

use std::f64::consts::{FRAC_PI_2, PI, TAU};

fn answer_angle(i: usize, n: usize) -> f64 {
    -FRAC_PI_2 + TAU * (i as f64) / (n as f64)
}

fn angular_distance(a: f64, b: f64) -> f64 {
    let mut d = (a - b).abs();
    if d > PI {
        d = TAU - d;
    }
    d
}

fn insertion_index(click_angle: f64, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    let phi = (click_angle + FRAC_PI_2).rem_euclid(TAU);
    let f = phi * n as f64 / TAU;
    (f.ceil() as usize).min(n)
}

#[component]
fn ClosedQuestionPage(
    topic_id: String,
    question: ReadSignal<Option<Question>>,
    set_question: WriteSignal<Option<Question>>,
) -> impl IntoView {
    let (knob_x, set_knob_x) = signal(0.0_f64);
    let (knob_y, set_knob_y) = signal(0.0_f64);
    let (dragging, set_dragging) = signal(false);
    let (did_drag, set_did_drag) = signal(false);

    let num_answers = Memo::new(move |_| {
        question.get().map(|q| q.answers.len()).unwrap_or(0)
    });

    // Opinion text — in the manner of Ben Jonson
    const BANDS: &[(f64, &str)] = &[
        (0.12, ""),
        (0.35, "My inclination tends towards {}"),
        (0.65, "I find myself persuaded by {}"),
        (1.01, "With settled conviction, I hold firmly with {}"),
    ];

    let opinion_text = Memo::new(move |_| {
        let q = question.get();
        let Some(q) = q.as_ref() else {
            return String::new();
        };
        let n = q.answers.len();

        if n == 0 {
            return "\u{2022} The circle stands empty, a stage awaiting its players. \
                    Pray, touch it, and set forth a position."
                .to_string();
        }
        if n == 1 {
            return "\u{2022} A solitary voice echoes \u{2014} yet true discourse \
                    demands a partner. Touch the circle once more."
                .to_string();
        }

        let x = knob_x.get();
        let y = knob_y.get();
        let dist = (x * x + y * y).sqrt();

        let band = BANDS.iter().find(|(max, _)| dist < *max).unwrap();
        if band.1.is_empty() {
            return "I remain unswayed, holding no fixed position in this matter.".to_string();
        }

        let angle = y.atan2(x);
        let mut scored: Vec<(usize, f64)> = (0..n)
            .map(|i| (i, angular_distance(angle, answer_angle(i, n))))
            .collect();
        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        let closest = &q.answers[scored[0].0];

        if n >= 2 && dist > 0.25 {
            let ratio = scored[0].1 / scored[1].1.max(0.001);
            if ratio > 0.7 {
                let second = &q.answers[scored[1].0];
                return format!(
                    "My judgement hangs divided betwixt {closest} and {second}."
                );
            }
        }

        format!("{}.", band.1.replace("{}", closest))
    });

    let on_knob_pointerdown = move |ev: web_sys::PointerEvent| {
        ev.stop_propagation();
        set_dragging.set(true);
        set_did_drag.set(false);
        let target = ev.current_target().unwrap();
        let el: &web_sys::Element = target.unchecked_ref();
        el.set_pointer_capture(ev.pointer_id()).ok();
    };

    let on_knob_pointermove = move |ev: web_sys::PointerEvent| {
        if !dragging.get_untracked() {
            return;
        }
        set_did_drag.set(true);
        let target = ev.current_target().unwrap();
        let el: web_sys::HtmlElement = target.unchecked_into();
        let circle = el.closest(".vote-circle").unwrap().unwrap();
        let rect = circle.get_bounding_client_rect();
        let cx = rect.left() + rect.width() / 2.0;
        let cy = rect.top() + rect.height() / 2.0;
        let radius = rect.width() / 2.0;
        let mut nx = (ev.client_x() as f64 - cx) / radius;
        let mut ny = (ev.client_y() as f64 - cy) / radius;
        let dist = (nx * nx + ny * ny).sqrt();
        if dist > 1.0 {
            nx /= dist;
            ny /= dist;
        }
        set_knob_x.set(nx);
        set_knob_y.set(ny);
    };

    let on_knob_pointerup = move |_ev: web_sys::PointerEvent| {
        set_dragging.set(false);
    };

    let qid = Memo::new(move |_| {
        question
            .get()
            .map(|q| q.id.clone())
            .unwrap_or_default()
    });
    let tid_add = topic_id.clone();
    let on_circle_click = move |ev: web_sys::MouseEvent| {
        if did_drag.get_untracked() {
            set_did_drag.set(false);
            return;
        }
        let target = ev.target().unwrap();
        let el: &web_sys::Element = target.unchecked_ref();
        if el.class_list().contains("knob") {
            return;
        }

        let circle_el = ev.current_target().unwrap();
        let circle: &web_sys::Element = circle_el.unchecked_ref();
        let rect = circle.get_bounding_client_rect();
        let cx = rect.left() + rect.width() / 2.0;
        let cy = rect.top() + rect.height() / 2.0;
        let click_angle = (ev.client_y() as f64 - cy).atan2(ev.client_x() as f64 - cx);
        let n = num_answers.get_untracked();
        let index = insertion_index(click_angle, n);

        let window = web_sys::window().unwrap();
        let text = window
            .prompt_with_message("What position shall here be voiced?")
            .ok()
            .flatten()
            .unwrap_or_default();
        if text.trim().is_empty() {
            return;
        }

        let tid = tid_add.clone();
        let qid = qid.get_untracked();
        wasm_bindgen_futures::spawn_local(async move {
            match api_post::<Question>(
                &format!("/api/topics/{tid}/questions/{qid}/answers"),
                &AddAnswer { text, index },
            )
            .await
            {
                Ok(q) => set_question.set(Some(q)),
                Err(e) => log!("[add_answer] {e}"),
            }
        });
    };

    let tid2 = topic_id.clone();

    view! {
        <div class="page question-page">
            <a href=format!("#/topic/{tid2}") class="back-link">"Return to questions"</a>
            <h2 class="question-title">{move || question.get().map(|q| q.text).unwrap_or_default()}</h2>

            <div class="opinion-text">{opinion_text}</div>

            <div class="vote-circle-container">
                <div class="vote-circle" on:click=on_circle_click>
                    {move || {
                        let q = question.get();
                        let Some(q) = q.as_ref() else { return Vec::new() };
                        let n = q.answers.len();
                        q.answers.iter().enumerate().map(|(i, answer)| {
                            let angle = answer_angle(i, n);
                            let label_r = 52.0;
                            let lx = 50.0 + label_r * angle.cos();
                            let ly = 50.0 + label_r * angle.sin();
                            let dot_r = 43.0;
                            let dx = 50.0 + dot_r * angle.cos();
                            let dy = 50.0 + dot_r * angle.sin();
                            view! {
                                <div
                                    class="answer-dot"
                                    style:left=format!("{dx}%")
                                    style:top=format!("{dy}%")
                                />
                                <div
                                    class="answer-label"
                                    style:left=format!("{lx}%")
                                    style:top=format!("{ly}%")
                                >
                                    {answer.clone()}
                                </div>
                            }
                        }).collect::<Vec<_>>()
                    }}

                    <Show when=move || { num_answers.get() >= 2 }>
                        <div
                            class="knob"
                            class:dragging=dragging
                            style:left=move || format!("{}%", 50.0 + knob_x.get() * 43.0)
                            style:top=move || format!("{}%", 50.0 + knob_y.get() * 43.0)
                            on:pointerdown=on_knob_pointerdown
                            on:pointermove=on_knob_pointermove
                            on:pointerup=on_knob_pointerup
                        />
                    </Show>
                </div>
            </div>
        </div>
    }
}

// ── Open Question: 2D Plane ─────────────────────────────────────────

#[component]
fn OpenQuestionPage(
    topic_id: String,
    question_id: String,
    question: ReadSignal<Option<Question>>,
    set_question: WriteSignal<Option<Question>>,
    current_user: ReadSignal<Option<User>>,
) -> impl IntoView {
    let (my_text, set_my_text) = signal(String::new());
    let (token_count, set_token_count) = signal(Option::<usize>::None);
    let (positions, set_positions) = signal(PlanePositions { points: vec![] });
    let debounce_handle = std::cell::Cell::new(0i32);
    let (all_users, set_all_users) = signal(Vec::<User>::new());

    // Reload users whenever question updates (new answers may come from new users)
    Effect::new(move || {
        let _ = question.get(); // track question changes
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(users) = api_get::<Vec<User>>("/api/users").await {
                set_all_users.set(users);
            }
        });
    });

    // Load this user's existing answer when user changes
    Effect::new(move || {
        let user = current_user.get(); // track user changes
        let q = question.get_untracked(); // don't track question (avoids re-trigger on submit)
        if let (Some(u), Some(q)) = (user, q) {
            if let Some(existing) = q.open_answers.iter().find(|a| a.user_name == u.name) {
                set_my_text.set(existing.text.clone());
            } else {
                set_my_text.set(String::new());
            }
        }
    });

    // Fetch positions whenever question updates (embeddings computed server-side)
    let tid_pos = topic_id.clone();
    let qid_pos = question_id.clone();
    Effect::new(move || {
        let _ = question.get(); // track question changes
        let tid = tid_pos.clone();
        let qid = qid_pos.clone();
        wasm_bindgen_futures::spawn_local(async move {
            // Small delay to let server-side background embedding finish
            gloo_timers::future::TimeoutFuture::new(200).await;
            if let Ok(pos) = api_get::<PlanePositions>(
                &format!("/api/topics/{tid}/questions/{qid}/positions"),
            )
            .await
            {
                set_positions.set(pos);
            }
        });
    });

    let tid = topic_id.clone();
    let qid = question_id.clone();
    let submit_debounced = move |text: String| {
        let Some(user) = current_user.get_untracked() else {
            return;
        };
        if text.trim().is_empty() {
            return;
        }
        let tid = tid.clone();
        let qid = qid.clone();
        wasm_bindgen_futures::spawn_local(async move {
            match api_post::<Question>(
                &format!("/api/topics/{tid}/questions/{qid}/open-answers"),
                &AddOpenAnswer {
                    user_name: user.name,
                    text,
                },
            )
            .await
            {
                Ok(q) => set_question.set(Some(q)),
                Err(e) => log!("[add_open_answer] {e}"),
            }
        });
    };

    let find_user = move |name: &str| -> Option<User> {
        all_users
            .get()
            .into_iter()
            .find(|u| u.name == name)
    };

    let tid2 = topic_id.clone();

    view! {
        <div class="page question-page open-question-page">
            <a href=format!("#/topic/{tid2}") class="back-link">"Return to questions"</a>
            <h2 class="question-title">{move || question.get().map(|q| q.text).unwrap_or_default()}</h2>

            <div class="open-answer-form">
                <div class="textarea-wrap">
                    <textarea
                        placeholder="Speak thy mind upon this matter..."
                        prop:value=my_text
                        on:input=move |ev| {
                            let val = event_target_value(&ev);
                            set_my_text.set(val.clone());
                            // Debounce: tokenize + submit
                            let prev = debounce_handle.get();
                            if prev != 0 {
                                web_sys::window().unwrap().clear_timeout_with_handle(prev);
                            }
                            if val.trim().is_empty() {
                                set_token_count.set(Some(0));
                                debounce_handle.set(0);
                                return;
                            }
                            let submit = submit_debounced.clone();
                            let cb = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
                                let val = val.clone();
                                // Tokenize
                                {
                                    let val = val.clone();
                                    wasm_bindgen_futures::spawn_local(async move {
                                        #[derive(serde::Deserialize)]
                                        struct TokenResp { num_tokens: usize }
                                        #[derive(serde::Serialize)]
                                        struct TokenReq { text: String }
                                        if let Ok(resp) = api_post::<TokenResp>(
                                            "/embedding/tokenize",
                                            &TokenReq { text: val },
                                        ).await {
                                            set_token_count.set(Some(resp.num_tokens));
                                        }
                                    });
                                }
                                // Submit
                                submit(val);
                            });
                            let handle = web_sys::window().unwrap()
                                .set_timeout_with_callback_and_timeout_and_arguments_0(
                                    cb.as_ref().unchecked_ref(), 500
                                ).unwrap_or(0);
                            cb.forget();
                            debounce_handle.set(handle);
                        }
                    />
                    <div class="token-counter" class:over-limit=move || token_count.get().map(|n| n > 128).unwrap_or(false)>
                        {move || match token_count.get() {
                            Some(n) => format!("{n}/128 tokens"),
                            None => "0/128 tokens".to_string(),
                        }}
                    </div>
                </div>
            </div>

            <div class="open-plane">
                <div class="plane-empty-text">
                    {move || {
                        let q = question.get();
                        let n = q.as_ref().map(|q| q.open_answers.len()).unwrap_or(0);
                        if n == 0 {
                            "The field lies fallow \u{2014} be the first to plant a thought.".to_string()
                        } else {
                            String::new()
                        }
                    }}
                </div>
                {move || {
                    let q = question.get();
                    let Some(q) = q.as_ref() else { return Vec::new() };
                    let pos = positions.get();
                    q.open_answers.iter().map(|answer| {
                        let user = find_user(&answer.user_name);
                        let (shape, color) = user
                            .map(|u| (u.shape, u.color))
                            .unwrap_or((Shape::Circle, "#808080".to_string()));
                        let pt = pos.points.iter().find(|p| p.user_name == answer.user_name);
                        let x_pct = pt.map(|p| p.x * 100.0).unwrap_or(50.0);
                        let y_pct = pt.map(|p| p.y * 100.0).unwrap_or(50.0);
                        let answer_text = answer.text.clone();
                        let user_name = answer.user_name.clone();
                        view! {
                            <div
                                class="plane-point"
                                style:left=format!("{x_pct}%")
                                style:top=format!("{y_pct}%")
                            >
                                <div class="plane-avatar">
                                    {shape_svg(shape, color, 32.0)}
                                    <div class="plane-tooltip">
                                        <strong>{user_name.clone()}</strong>
                                        <br/>
                                        {answer_text}
                                    </div>
                                </div>
                                <span class="plane-name">{user_name}</span>
                            </div>
                        }
                    }).collect::<Vec<_>>()
                }}
            </div>
        </div>
    }
}
