use leptos::prelude::*;
use mace_reforge_shared::*;

use crate::api::*;

#[component]
pub fn TopicPage(topic_id: String) -> impl IntoView {
    let (topic, set_topic) = signal(Option::<TopicWithCount>::None);
    let (questions, set_questions) = signal(Vec::<Question>::new());
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
    let do_create = move |kind: QuestionKind| {
        let window = web_sys::window().unwrap();
        let text = match window.prompt_with_message("What matter shall be put to question?").ok().flatten() {
            Some(t) if !t.trim().is_empty() => t,
            _ => return,
        };
        let tid = tid2.clone();
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

    let tid3 = topic_id.clone();

    view! {
        <div class="page topic-page">
            <a href="#/" class="back-link">"Return to discourses"</a>
            <div class="page-header">
                <h1>{move || topic.get().map(|t| t.title).unwrap_or_default()}</h1>
                <div class="add-btn-group">
                    {
                        let do_create = do_create.clone();
                        view! {
                            <button class="add-btn" on:click=move |_| do_create(QuestionKind::Closed) title="New closed question">"+ Closed"</button>
                        }
                    }
                    <button class="add-btn" on:click=move |_| do_create(QuestionKind::Open) title="New open question">"+ Open"</button>
                </div>
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
                        let del_tid = tid.clone();
                        let del_qid = qid.clone();
                        let do_delete = move |ev: web_sys::MouseEvent| {
                            ev.prevent_default();
                            ev.stop_propagation();
                            let window = web_sys::window().unwrap();
                            if window.confirm_with_message("Remove this question?").unwrap_or(false) {
                                let tid = del_tid.clone();
                                let qid = del_qid.clone();
                                wasm_bindgen_futures::spawn_local(async move {
                                    match api_delete(&format!("/api/topics/{tid}/questions/{qid}")).await {
                                        Ok(()) => set_questions.update(|qs| qs.retain(|q| q.id != qid)),
                                        Err(e) => log!("[delete_question] {e}"),
                                    }
                                });
                            }
                        };
                        view! {
                            <a class="question-card" href=format!("#/topic/{tid}/question/{qid}")>
                                <span class="question-text">{question.text}</span>
                                <span class="question-meta">
                                    <span class="kind-label">{kind_label}</span>
                                    <span class="answer-count">{subtitle}</span>
                                    <button class="delete-btn" on:click=do_delete title="Delete question">{"\u{00D7}"}</button>
                                </span>
                            </a>
                        }
                    }
                </For>
            </div>
        </div>
    }
}
