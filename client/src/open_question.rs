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

    // Axis controls: local editing state
    let (x_neg, set_x_neg) = signal(String::new());
    let (x_pos, set_x_pos) = signal(String::new());
    let (y_neg, set_y_neg) = signal(String::new());
    let (y_pos, set_y_pos) = signal(String::new());

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

    // Sync axis inputs from server state
    Effect::new(move || {
        let q = question.get();
        if let Some(q) = q.as_ref() {
            match &q.x_axis {
                Some((neg, pos)) => {
                    set_x_neg.set(neg.clone());
                    set_x_pos.set(pos.clone());
                }
                None => {
                    set_x_neg.set(String::new());
                    set_x_pos.set(String::new());
                }
            }
            match &q.y_axis {
                Some((neg, pos)) => {
                    set_y_neg.set(neg.clone());
                    set_y_pos.set(pos.clone());
                }
                None => {
                    set_y_neg.set(String::new());
                    set_y_pos.set(String::new());
                }
            }
        }
    });

    // Fetch positions whenever question or local axes change (debounced)
    let tid_pos = topic_id.clone();
    let qid_pos = question_id.clone();
    let pos_debounce = std::cell::Cell::new(0i32);
    Effect::new(move || {
        let _ = question.get();
        let xn = x_neg.get();
        let xp = x_pos.get();
        let yn = y_neg.get();
        let yp = y_pos.get();

        let prev = pos_debounce.get();
        if prev != 0 {
            web_sys::window().unwrap().clear_timeout_with_handle(prev);
        }

        let tid = tid_pos.clone();
        let qid = qid_pos.clone();
        let cb = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
            let x_axis = if xn.trim().is_empty() || xp.trim().is_empty() {
                None
            } else {
                Some((xn.clone(), xp.clone()))
            };
            let y_axis = if yn.trim().is_empty() || yp.trim().is_empty() {
                None
            } else {
                Some((yn.clone(), yp.clone()))
            };
            let tid = tid.clone();
            let qid = qid.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(pos) = api_post::<PlanePositions>(
                    &format!("/api/topics/{tid}/questions/{qid}/positions"),
                    &SetAxes { x_axis, y_axis },
                )
                .await
                {
                    set_positions.set(pos);
                    set_positions_ready.set(true);
                }
            });
        });
        let handle = web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                cb.as_ref().unchecked_ref(),
                300,
            )
            .unwrap_or(0);
        cb.forget();
        pos_debounce.set(handle);
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

    // Axis send closures
    let tid_ax = topic_id.clone();
    let qid_ax = question_id.clone();
    let send_x = move || {
        let neg = x_neg.get_untracked();
        let pos = x_pos.get_untracked();
        let y = question.get_untracked().and_then(|q| q.y_axis);
        let x = if neg.trim().is_empty() && pos.trim().is_empty() {
            None
        } else {
            Some((neg, pos))
        };
        let tid = tid_ax.clone();
        let qid = qid_ax.clone();
        wasm_bindgen_futures::spawn_local(async move {
            match api_post::<Question>(
                &format!("/api/topics/{tid}/questions/{qid}/axes"),
                &SetAxes { x_axis: x, y_axis: y },
            )
            .await
            {
                Ok(q) => set_question.set(Some(q)),
                Err(e) => log!("[set_axes] {e}"),
            }
        });
    };

    let tid_ax2 = topic_id.clone();
    let qid_ax2 = question_id.clone();
    let send_y = move || {
        let neg = y_neg.get_untracked();
        let pos = y_pos.get_untracked();
        let x = question.get_untracked().and_then(|q| q.x_axis);
        let y = if neg.trim().is_empty() && pos.trim().is_empty() {
            None
        } else {
            Some((neg, pos))
        };
        let tid = tid_ax2.clone();
        let qid = qid_ax2.clone();
        wasm_bindgen_futures::spawn_local(async move {
            match api_post::<Question>(
                &format!("/api/topics/{tid}/questions/{qid}/axes"),
                &SetAxes { x_axis: x, y_axis: y },
            )
            .await
            {
                Ok(q) => set_question.set(Some(q)),
                Err(e) => log!("[set_axes] {e}"),
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

            <div class="axis-controls">
                <div class="axis-row">
                    <span class="axis-label">"x"</span>
                    <input
                        class="axis-input"
                        placeholder="negative..."
                        prop:value=x_neg
                        on:input=move |ev| set_x_neg.set(event_target_value(&ev))
                    />
                    <input
                        class="axis-input"
                        placeholder="positive..."
                        prop:value=x_pos
                        on:input=move |ev| set_x_pos.set(event_target_value(&ev))
                    />
                    <button class="axis-send-btn" on:click=move |_| send_x()>"Send"</button>
                </div>
                <div class="axis-row">
                    <span class="axis-label">"y"</span>
                    <input
                        class="axis-input"
                        placeholder="negative..."
                        prop:value=y_neg
                        on:input=move |ev| set_y_neg.set(event_target_value(&ev))
                    />
                    <input
                        class="axis-input"
                        placeholder="positive..."
                        prop:value=y_pos
                        on:input=move |ev| set_y_pos.set(event_target_value(&ev))
                    />
                    <button class="axis-send-btn" on:click=move |_| send_y()>"Send"</button>
                </div>
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
