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

// ── Routing ──────────────────────────────────────────────────────────

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

// ── Entry ────────────────────────────────────────────────────────────

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let (route, set_route) = signal(parse_hash());

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
        </header>
        <main>
            {move || match route.get() {
                Route::Home => view! { <HomePage/> }.into_any(),
                Route::Topic(id) => view! { <TopicPage topic_id=id/> }.into_any(),
                Route::Question(tid, qid) => view! { <QuestionPage topic_id=tid question_id=qid/> }.into_any(),
            }}
        </main>
    }
}

// ── Home Page: Topic Grid ────────────────────────────────────────────

#[component]
fn HomePage() -> impl IntoView {
    let (topics, set_topics) = signal(Vec::<TopicWithCount>::new());
    let (new_title, set_new_title) = signal(String::new());
    let (error, set_error) = signal(Option::<String>::None);

    // Load topics
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

// ── Topic Page: Questions List ───────────────────────────────────────

#[component]
fn TopicPage(topic_id: String) -> impl IntoView {
    let (topic, set_topic) = signal(Option::<TopicWithCount>::None);
    let (questions, set_questions) = signal(Vec::<Question>::new());
    let (new_text, set_new_text) = signal(String::new());

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
        wasm_bindgen_futures::spawn_local(async move {
            match api_post::<Question>(&format!("/api/topics/{tid}/questions"), &CreateQuestion { text }).await {
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
                        let n = question.answers.len();
                        let subtitle = if n == 0 {
                            "yet unvoiced".to_string()
                        } else {
                            format!("{n} positions voiced")
                        };
                        view! {
                            <a class="question-card" href=format!("#/topic/{tid}/question/{qid}")>
                                <span class="question-text">{question.text}</span>
                                <span class="answer-count">{subtitle}</span>
                            </a>
                        }
                    }
                </For>
            </div>
        </div>
    }
}

// ── Question Page: Voting Circle ─────────────────────────────────────

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
fn QuestionPage(topic_id: String, question_id: String) -> impl IntoView {
    let (question, set_question) = signal(Option::<Question>::None);
    let (knob_x, set_knob_x) = signal(0.0_f64);
    let (knob_y, set_knob_y) = signal(0.0_f64);
    let (dragging, set_dragging) = signal(false);
    let (did_drag, set_did_drag) = signal(false);

    let tid = topic_id.clone();
    let qid = question_id.clone();
    Effect::new(move || {
        let tid = tid.clone();
        let qid = qid.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(q) = api_get::<Question>(&format!("/api/topics/{tid}/questions/{qid}")).await {
                set_question.set(Some(q));
            }
        });
    });

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
        let Some(q) = q.as_ref() else { return String::new() };
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

    // Knob drag — only on the knob element
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
        let circle = el
            .closest(".vote-circle")
            .unwrap()
            .unwrap();
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

    // Click on circle → add answer
    let tid_add = topic_id.clone();
    let qid_add = question_id.clone();
    let on_circle_click = move |ev: web_sys::MouseEvent| {
        if did_drag.get_untracked() {
            set_did_drag.set(false);
            return;
        }
        // Don't add if clicking on the knob
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
        let qid = qid_add.clone();
        wasm_bindgen_futures::spawn_local(async move {
            match api_post::<Question>(
                &format!("/api/topics/{tid}/questions/{qid}/answers"),
                &AddAnswer {
                    text,
                    index,
                },
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
