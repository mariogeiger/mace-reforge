use leptos::prelude::*;
use mace_reforge_shared::*;
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use wasm_bindgen::JsCast;

use crate::api::*;
use crate::shapes::shape_svg;

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
pub fn ClosedQuestionPage(
    topic_id: String,
    question: ReadSignal<Option<Question>>,
    set_question: WriteSignal<Option<Question>>,
    current_user: ReadSignal<Option<User>>,
) -> impl IntoView {
    let (knob_x, set_knob_x) = signal(0.0_f64);
    let (knob_y, set_knob_y) = signal(0.0_f64);
    let (dragging, set_dragging) = signal(false);
    let (did_drag, set_did_drag) = signal(false);
    let (all_users, set_all_users) = signal(Vec::<User>::new());

    // Load users for avatar rendering
    Effect::new(move || {
        let _ = question.get();
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(users) = api_get::<Vec<User>>("/api/users").await {
                set_all_users.set(users);
            }
        });
    });

    // Initialize knob position from existing vote
    Effect::new(move || {
        let user = current_user.get();
        let q = question.get_untracked();
        if let (Some(u), Some(q)) = (user, q) {
            if let Some(v) = q.votes.iter().find(|v| v.user_name == u.name) {
                set_knob_x.set(v.x);
                set_knob_y.set(v.y);
            } else {
                set_knob_x.set(0.0);
                set_knob_y.set(0.0);
            }
        }
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

    let qid = Memo::new(move |_| {
        question
            .get()
            .map(|q| q.id.clone())
            .unwrap_or_default()
    });

    // Debounced vote save
    let vote_debounce = std::cell::Cell::new(0i32);
    let tid_vote = topic_id.clone();
    let save_vote = move || {
        let prev = vote_debounce.get();
        if prev != 0 {
            web_sys::window().unwrap().clear_timeout_with_handle(prev);
        }
        let tid = tid_vote.clone();
        let cb = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
            let Some(user) = current_user.get_untracked() else { return };
            let x = knob_x.get_untracked();
            let y = knob_y.get_untracked();
            let tid = tid.clone();
            let qid = qid.get_untracked();
            wasm_bindgen_futures::spawn_local(async move {
                match api_post::<Question>(
                    &format!("/api/topics/{tid}/questions/{qid}/votes"),
                    &CastVote { user_name: user.name, x, y },
                ).await {
                    Ok(q) => set_question.set(Some(q)),
                    Err(e) => log!("[cast_vote] {e}"),
                }
            });
        });
        let handle = web_sys::window().unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                cb.as_ref().unchecked_ref(), 300
            ).unwrap_or(0);
        cb.forget();
        vote_debounce.set(handle);
    };

    // Drag: mousemove on the circle sets avatar to mouse position, clamped to radius.
    let save_vote_up = save_vote.clone();
    let on_circle_mousemove = move |ev: web_sys::MouseEvent| {
        if !dragging.get_untracked() { return; }
        set_did_drag.set(true);
        let circle: web_sys::Element = ev.current_target().unwrap().unchecked_into();
        let rect = circle.get_bounding_client_rect();
        let cx = rect.left() + rect.width() / 2.0;
        let cy = rect.top() + rect.height() / 2.0;
        let display_radius = rect.width() / 2.0 * 0.43;
        let mut nx = (ev.client_x() as f64 - cx) / display_radius;
        let mut ny = (ev.client_y() as f64 - cy) / display_radius;
        let dist = (nx * nx + ny * ny).sqrt();
        if dist > 1.0 { nx /= dist; ny /= dist; }
        set_knob_x.set(nx);
        set_knob_y.set(ny);
    };

    let on_circle_mousedown = move |ev: web_sys::MouseEvent| {
        ev.prevent_default();
        set_dragging.set(true);
        set_did_drag.set(false);
    };

    let on_circle_mouseup = move |_ev: web_sys::MouseEvent| {
        if dragging.get_untracked() {
            set_dragging.set(false);
            save_vote_up();
        }
    };

    let on_circle_mouseleave = move |_ev: web_sys::MouseEvent| {
        if dragging.get_untracked() {
            set_dragging.set(false);
            save_vote.clone()();
        }
    };

    // Click on circle → add answer (only if not dragging)
    let tid_add = topic_id.clone();
    let on_circle_click = move |ev: web_sys::MouseEvent| {
        if did_drag.get_untracked() {
            set_did_drag.set(false);
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
                <div
                    class="vote-circle"
                    on:click=on_circle_click
                    on:mousedown=on_circle_mousedown
                    on:mousemove=on_circle_mousemove
                    on:mouseup=on_circle_mouseup
                    on:mouseleave=on_circle_mouseleave
                >
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

                    // Other users' vote avatars
                    {move || {
                        let q = question.get();
                        let Some(q) = q.as_ref() else { return Vec::new() };
                        let my_name = current_user.get().map(|u| u.name).unwrap_or_default();
                        q.votes.iter().filter(|v| v.user_name != my_name).map(|vote| {
                            let user = all_users.get().into_iter().find(|u| u.name == vote.user_name);
                            let (shape, color) = user
                                .map(|u| (u.shape, u.color))
                                .unwrap_or((Shape::Circle, "#808080".to_string()));
                            let lx = 50.0 + vote.x * 43.0;
                            let ly = 50.0 + vote.y * 43.0;
                            let name = vote.user_name.clone();
                            view! {
                                <div
                                    class="vote-avatar"
                                    style:left=format!("{lx}%")
                                    style:top=format!("{ly}%")
                                    title=name
                                >
                                    {shape_svg(shape, color, 26.0)}
                                </div>
                            }
                        }).collect::<Vec<_>>()
                    }}

                    // Current user's avatar (on top, with border)
                    {move || {
                        let user = current_user.get();
                        let Some(u) = user else { return None };
                        let lx = 50.0 + knob_x.get() * 43.0;
                        let ly = 50.0 + knob_y.get() * 43.0;
                        Some(view! {
                            <div
                                class="vote-avatar vote-avatar-me"
                                class:dragging=dragging
                                style:left=format!("{lx}%")
                                style:top=format!("{ly}%")
                            >
                                {shape_svg(u.shape, u.color, 32.0)}
                            </div>
                        })
                    }}
                </div>
            </div>
        </div>
    }
}
