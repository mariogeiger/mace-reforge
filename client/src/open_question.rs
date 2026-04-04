use leptos::prelude::*;
use mace_reforge_shared::*;
use wasm_bindgen::JsCast;

use crate::api::*;
use crate::shapes::shape_svg;

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

#[component]
pub fn OpenQuestionPage(
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
    let (positions_ready, set_positions_ready) = signal(false);

    // Reload users whenever question updates (new answers may come from new users)
    Effect::new(move || {
        let _ = question.get();
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(users) = api_get::<Vec<User>>("/api/users").await {
                set_all_users.set(users);
            }
        });
    });

    // Load this user's existing answer when user changes
    Effect::new(move || {
        let user = current_user.get();
        let q = question.get_untracked();
        if let (Some(u), Some(q)) = (user, q) {
            if let Some(existing) = q.open_answers.iter().find(|a| a.user_name == u.name) {
                let text = existing.text.clone();
                set_my_text.set(text.clone());
                if !text.trim().is_empty() {
                    wasm_bindgen_futures::spawn_local(async move {
                        #[derive(serde::Deserialize)]
                        struct TokenResp { num_tokens: usize }
                        #[derive(serde::Serialize)]
                        struct TokenReq { text: String }
                        if let Ok(resp) = api_post::<TokenResp>(
                            "/embedding/tokenize",
                            &TokenReq { text },
                        ).await {
                            set_token_count.set(Some(resp.num_tokens));
                        }
                    });
                }
            } else {
                set_my_text.set(String::new());
                set_token_count.set(Some(0));
            }
        }
    });

    // Fetch positions whenever question updates (embeddings computed server-side)
    let tid_pos = topic_id.clone();
    let qid_pos = question_id.clone();
    Effect::new(move || {
        let _ = question.get();
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
                set_positions_ready.set(true);
            }
        });
    });

    // ── WebSocket connection ────────────────────────────────────────
    let url = ws_url(&topic_id, &question_id);
    Effect::new(move || {
        let Ok(socket) = web_sys::WebSocket::new(&url) else {
            return;
        };
        let on_message = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::MessageEvent)>::new(
            move |ev: web_sys::MessageEvent| {
                let Some(text) = ev.data().as_string() else { return };
                let Ok(msg) = serde_json::from_str::<WsMsg>(&text) else { return };
                if let WsMsg::QuestionUpdated { question } = msg {
                    set_question.set(Some(question));
                }
            },
        );
        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();
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
        <div class="open-question-page">
            <a href=format!("#/topic/{tid2}") class="back-link">"Return to questions"</a>
            <h2 class="question-title">{move || question.get().map(|q| q.text).unwrap_or_default()}</h2>

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
                    if !positions_ready.get() { return Vec::new() }
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

            <div class="open-answer-form">
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
    }
}
