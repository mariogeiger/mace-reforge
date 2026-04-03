use leptos::prelude::*;
use mace_reforge_shared::*;
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use wasm_bindgen::JsCast;

use crate::api::*;
use crate::shapes::shape_svg;

// ── Geometry ────────────────────────────────────────────────────────

/// Answers and vote avatars sit at this % of circle-element width from center.
const R: f64 = 43.0;
/// Labels sit slightly further out.
const LABEL_R: f64 = 52.0;
/// Pixel radius for avatar hit-testing on pointer down.
const HIT_PX: f64 = 24.0;

fn answer_angle(i: usize, n: usize) -> f64 {
    -FRAC_PI_2 + TAU * (i as f64) / (n as f64)
}

fn angular_distance(a: f64, b: f64) -> f64 {
    ((a - b + PI).rem_euclid(TAU) - PI).abs()
}

fn insertion_index(click_angle: f64, n: usize) -> usize {
    let phi = (click_angle + FRAC_PI_2).rem_euclid(TAU);
    ((phi * n as f64 / TAU).ceil() as usize).min(n)
}

/// (center_x_px, center_y_px, scale_px) where scale converts +-1 normalised coords to pixels.
fn circle_metrics(el: &web_sys::Element) -> (f64, f64, f64) {
    let r = el.get_bounding_client_rect();
    (
        r.left() + r.width() / 2.0,
        r.top() + r.height() / 2.0,
        r.width() * R / 100.0,
    )
}

/// Client pixel coords -> normalised circle coords, clamped to the unit disc.
fn to_normalised(px: f64, py: f64, el: &web_sys::Element) -> (f64, f64) {
    let (cx, cy, s) = circle_metrics(el);
    let (nx, ny) = ((px - cx) / s, (py - cy) / s);
    let d = (nx * nx + ny * ny).sqrt().max(1.0);
    (nx / d, ny / d)
}

// ── Opinion text (Ben Jonson voice) ─────────────────────────────────

/// Static messages when fewer than 2 answers exist (indexed by answer count).
const FEW_ANSWERS: &[&str] = &[
    "The circle stands empty, a stage awaiting its players. \
     Pray, touch it, and set forth a position.",
    "A solitary voice echoes \u{2014} yet true discourse \
     demands a partner. Touch the circle once more.",
];

/// Distance-from-center → conviction template. The first entry has no `{}`
/// so `replace("{}", x)` is a no-op, producing the "unswayed" text directly.
const BANDS: &[(f64, &str)] = &[
    (0.12, "I remain unswayed, holding no fixed position in this matter."),
    (0.35, "My inclination tends towards {}."),
    (0.65, "I find myself persuaded by {}."),
    (1.01, "With settled conviction, I hold firmly with {}."),
];

fn opinion(q: &Question, x: f64, y: f64) -> String {
    let n = q.answers.len();
    if n < FEW_ANSWERS.len() {
        return FEW_ANSWERS[n].into();
    }

    let dist = (x * x + y * y).sqrt();
    let band = BANDS.iter().find(|(max, _)| dist < *max).unwrap().1;

    let angle = y.atan2(x);
    let mut scored: Vec<(usize, f64)> = (0..n)
        .map(|i| (i, angular_distance(angle, answer_angle(i, n))))
        .collect();
    scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    if dist > 0.25 && scored[0].1 / scored[1].1.max(0.001) > 0.7 {
        let (a, b) = (&q.answers[scored[0].0], &q.answers[scored[1].0]);
        return format!("My judgement hangs divided betwixt {a} and {b}.");
    }

    band.replace("{}", &q.answers[scored[0].0])
}

// ── WebSocket helpers ───────────────────────────────────────────────

fn ws_url(topic_id: &str, question_id: &str) -> String {
    let loc = web_sys::window().unwrap().location();
    let protocol = if loc.protocol().unwrap_or_default() == "https:" {
        "wss:"
    } else {
        "ws:"
    };
    let host = loc.host().unwrap_or_default();
    format!("{protocol}//{host}/api/topics/{topic_id}/questions/{question_id}/ws")
}

fn ws_send(ws: &ReadSignal<Option<web_sys::WebSocket>>, msg: &WsMsg) {
    if let Some(socket) = ws.get_untracked() {
        if socket.ready_state() == 1 {
            if let Ok(json) = serde_json::to_string(msg) {
                let _ = socket.send_with_str(&json);
            }
        }
    }
}

// ── Component ───────────────────────────────────────────────────────

#[component]
pub fn ClosedQuestionPage(
    topic_id: String,
    question_id: String,
    question: ReadSignal<Option<Question>>,
    set_question: WriteSignal<Option<Question>>,
    current_user: ReadSignal<Option<User>>,
) -> impl IntoView {
    let (knob_x, set_knob_x) = signal(0.0_f64);
    let (knob_y, set_knob_y) = signal(0.0_f64);
    let (dragging, set_dragging) = signal(false);
    let (did_drag, set_did_drag) = signal(false);
    let (all_users, set_all_users) = signal(Vec::<User>::new());
    let (ws, set_ws) = signal(Option::<web_sys::WebSocket>::None);

    // ── Effects ─────────────────────────────────────────────────────

    Effect::new(move || {
        let _ = question.get();
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(users) = api_get::<Vec<User>>("/api/users").await {
                set_all_users.set(users);
            }
        });
    });

    Effect::new(move || {
        if let (Some(u), Some(q)) = (current_user.get(), question.get_untracked()) {
            if let Some(v) = q.votes.iter().find(|v| v.user_name == u.name) {
                set_knob_x.set(v.x);
                set_knob_y.set(v.y);
            } else {
                set_knob_x.set(0.0);
                set_knob_y.set(0.0);
            }
        }
    });

    // ── WebSocket connection ────────────────────────────────────────

    let url = ws_url(&topic_id, &question_id);
    Effect::new(move || {
        let Ok(socket) = web_sys::WebSocket::new(&url) else {
            return;
        };

        let on_message = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::MessageEvent)>::new(
            move |ev: web_sys::MessageEvent| {
                let Some(text) = ev.data().as_string() else {
                    return;
                };
                let Ok(msg) = serde_json::from_str::<WsMsg>(&text) else {
                    return;
                };
                let my_name = current_user
                    .get_untracked()
                    .map(|u| u.name)
                    .unwrap_or_default();
                match msg {
                    WsMsg::VoteMoved { user_name, x, y } => {
                        if user_name != my_name {
                            set_question.update(|q| {
                                if let Some(q) = q {
                                    if let Some(v) =
                                        q.votes.iter_mut().find(|v| v.user_name == user_name)
                                    {
                                        v.x = x;
                                        v.y = y;
                                    } else {
                                        q.votes.push(Vote { user_name, x, y });
                                    }
                                }
                            });
                        }
                    }
                    WsMsg::QuestionUpdated { question } => {
                        set_question.set(Some(question));
                    }
                }
            },
        );
        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();

        set_ws.set(Some(socket));
    });

    on_cleanup(move || {
        if let Some(socket) = ws.get_untracked() {
            let _ = socket.close();
        }
    });

    // ── Derived state ───────────────────────────────────────────────

    let num_answers = Memo::new(move |_| {
        question.get().map(|q| q.answers.len()).unwrap_or(0)
    });

    let qid = Memo::new(move |_| {
        question
            .get()
            .map(|q| q.id.clone())
            .unwrap_or_default()
    });

    let opinion_text = Memo::new(move |_| {
        question
            .get()
            .map(|q| opinion(&q, knob_x.get(), knob_y.get()))
            .unwrap_or_default()
    });

    // ── Debounced vote save ─────────────────────────────────────────

    let vote_timer = std::cell::Cell::new(0i32);
    let tid_vote = topic_id.clone();
    let save_vote = move || {
        let prev = vote_timer.get();
        if prev != 0 {
            web_sys::window().unwrap().clear_timeout_with_handle(prev);
        }
        let tid = tid_vote.clone();
        let cb = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
            let Some(user) = current_user.get_untracked() else {
                return;
            };
            let (tid, qid) = (tid.clone(), qid.get_untracked());
            let (x, y) = (knob_x.get_untracked(), knob_y.get_untracked());
            wasm_bindgen_futures::spawn_local(async move {
                match api_post::<Question>(
                    &format!("/api/topics/{tid}/questions/{qid}/votes"),
                    &CastVote {
                        user_name: user.name,
                        x,
                        y,
                    },
                )
                .await
                {
                    Ok(q) => set_question.set(Some(q)),
                    Err(e) => log!("[cast_vote] {e}"),
                }
            });
        });
        let h = web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                cb.as_ref().unchecked_ref(),
                300,
            )
            .unwrap_or(0);
        cb.forget();
        vote_timer.set(h);
    };

    // ── Pointer events (unified mouse + touch via setPointerCapture) ─

    let on_pointerdown = move |ev: web_sys::PointerEvent| {
        if current_user.get_untracked().is_none() {
            return;
        }
        let el: web_sys::Element = ev.current_target().unwrap().unchecked_into();
        let (cx, cy, s) = circle_metrics(&el);
        let dx = ev.client_x() as f64 - (cx + knob_x.get_untracked() * s);
        let dy = ev.client_y() as f64 - (cy + knob_y.get_untracked() * s);
        if (dx * dx + dy * dy).sqrt() < HIT_PX {
            ev.prevent_default();
            let _ = el.set_pointer_capture(ev.pointer_id());
            set_dragging.set(true);
            set_did_drag.set(true);
        }
    };

    let on_pointermove = move |ev: web_sys::PointerEvent| {
        if !dragging.get_untracked() {
            return;
        }
        let el: web_sys::Element = ev.current_target().unwrap().unchecked_into();
        let (nx, ny) = to_normalised(ev.client_x() as f64, ev.client_y() as f64, &el);
        set_knob_x.set(nx);
        set_knob_y.set(ny);

        // Broadcast live position to other clients
        if let Some(user) = current_user.get_untracked() {
            ws_send(
                &ws,
                &WsMsg::VoteMoved {
                    user_name: user.name,
                    x: nx,
                    y: ny,
                },
            );
        }
    };

    let on_pointerup = move |ev: web_sys::PointerEvent| {
        if !dragging.get_untracked() {
            return;
        }
        set_dragging.set(false);
        let el: web_sys::Element = ev.current_target().unwrap().unchecked_into();
        let _ = el.release_pointer_capture(ev.pointer_id());
        save_vote();
    };

    // Click on empty circle -> add answer
    let tid_add = topic_id.clone();
    let on_click = move |ev: web_sys::MouseEvent| {
        if did_drag.get_untracked() {
            set_did_drag.set(false);
            return;
        }
        let el: web_sys::Element = ev.current_target().unwrap().unchecked_into();
        let r = el.get_bounding_client_rect();
        let angle = (ev.client_y() as f64 - r.top() - r.height() / 2.0)
            .atan2(ev.client_x() as f64 - r.left() - r.width() / 2.0);
        let idx = insertion_index(angle, num_answers.get_untracked());

        let text = web_sys::window()
            .unwrap()
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
                &AddAnswer { text, index: idx },
            )
            .await
            {
                Ok(q) => set_question.set(Some(q)),
                Err(e) => log!("[add_answer] {e}"),
            }
        });
    };

    // ── View ────────────────────────────────────────────────────────

    let tid2 = topic_id.clone();

    view! {
        <div class="page question-page">
            <a href=format!("#/topic/{tid2}") class="back-link">"Return to questions"</a>
            <h2 class="question-title">{move || question.get().map(|q| q.text).unwrap_or_default()}</h2>
            <div class="opinion-text">{opinion_text}</div>

            <div class="vote-circle-container">
                <div class="vote-circle"
                    on:click=on_click
                    on:pointerdown=on_pointerdown
                    on:pointermove=on_pointermove
                    on:pointerup=on_pointerup
                >
                    // Answer dots and labels
                    {move || {
                        let Some(q) = question.get() else { return Vec::new() };
                        let n = q.answers.len();
                        q.answers.iter().enumerate().map(|(i, ans)| {
                            let a = answer_angle(i, n);
                            let (dx, dy) = (50.0 + R * a.cos(), 50.0 + R * a.sin());
                            let (lx, ly) = (50.0 + LABEL_R * a.cos(), 50.0 + LABEL_R * a.sin());
                            view! {
                                <div class="answer-dot"
                                    style:left=format!("{dx}%") style:top=format!("{dy}%") />
                                <div class="answer-label"
                                    style:left=format!("{lx}%") style:top=format!("{ly}%")>
                                    {ans.clone()}
                                </div>
                            }
                        }).collect::<Vec<_>>()
                    }}

                    // Other users' avatars
                    {move || {
                        let Some(q) = question.get() else { return Vec::new() };
                        let me = current_user.get().map(|u| u.name).unwrap_or_default();
                        q.votes.iter().filter(|v| v.user_name != me).map(|v| {
                            let (shape, color) = all_users.get().into_iter()
                                .find(|u| u.name == v.user_name)
                                .map(|u| (u.shape, u.color))
                                .unwrap_or((Shape::Circle, "#808080".into()));
                            view! {
                                <div class="vote-avatar"
                                    style:left=format!("{}%", 50.0 + v.x * R)
                                    style:top=format!("{}%", 50.0 + v.y * R)
                                    title=v.user_name.clone()>
                                    {shape_svg(shape, color, 26.0)}
                                </div>
                            }
                        }).collect::<Vec<_>>()
                    }}

                    // Current user's draggable avatar
                    {move || {
                        let u = current_user.get()?;
                        Some(view! {
                            <div class="vote-avatar vote-avatar-me"
                                class:dragging=dragging
                                style:left=format!("{}%", 50.0 + knob_x.get() * R)
                                style:top=format!("{}%", 50.0 + knob_y.get() * R)>
                                {shape_svg(u.shape, u.color, 32.0)}
                            </div>
                        })
                    }}
                </div>
            </div>
        </div>
    }
}
