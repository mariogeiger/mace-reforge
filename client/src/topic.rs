use leptos::prelude::*;
use mace_reforge_shared::*;

use crate::api::*;

#[component]
pub fn TopicPage(topic_id: String) -> impl IntoView {
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
    let do_create = move |kind: QuestionKind| {
        let text = new_text.get_untracked();
        if text.trim().is_empty() {
            return;
        }
        set_new_text.set(String::new());
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

    let do_create_k = do_create.clone();
    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        if ev.key() == "Enter" {
            do_create_k(QuestionKind::Closed);
        }
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
                {
                    let do_create = do_create.clone();
                    view! {
                        <button on:click=move |_| do_create(QuestionKind::Closed)>"Closed"</button>
                    }
                }
                <button on:click=move |_| do_create(QuestionKind::Open)>"Open"</button>
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
