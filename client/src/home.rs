use leptos::prelude::*;
use mace_reforge_shared::*;

use crate::api::*;
use crate::Star;

#[component]
pub fn HomePage() -> impl IntoView {
    let (topics, set_topics) = signal(Vec::<TopicWithCount>::new());
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

    let do_create = move |_: web_sys::MouseEvent| {
        let window = web_sys::window().unwrap();
        let title = match window.prompt_with_message("Name thy discourse:").ok().flatten() {
            Some(t) if !t.trim().is_empty() => t,
            _ => return,
        };
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

    view! {
        <div class="page home-page">
            <div class="page-header">
                <h1>"Discourses"</h1>
                <button class="add-btn" on:click=do_create title="New discourse">"+"</button>
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
                    {
                        let tid = topic.id.clone();
                        let set_topics = set_topics.clone();
                        let do_delete = move |ev: web_sys::MouseEvent| {
                            ev.prevent_default();
                            ev.stop_propagation();
                            let window = web_sys::window().unwrap();
                            if window.confirm_with_message("Remove this discourse and all its questions?").unwrap_or(false) {
                                let tid = tid.clone();
                                let set_topics = set_topics.clone();
                                wasm_bindgen_futures::spawn_local(async move {
                                    match api_delete(&format!("/api/topics/{tid}")).await {
                                        Ok(()) => set_topics.update(|t| t.retain(|topic| topic.id != tid)),
                                        Err(e) => log!("[delete_topic] {e}"),
                                    }
                                });
                            }
                        };
                        view! {
                            <a class="topic-card" href=format!("#/topic/{}", topic.id)>
                                <Star class_name="card-star"/>
                                <span class="card-title">{topic.title.clone()}</span>
                                <span class="card-count">{topic.question_count}" questions within"</span>
                                <button class="delete-btn" on:click=do_delete title="Delete discourse">{"\u{00D7}"}</button>
                            </a>
                        }
                    }
                </For>
            </div>
        </div>
    }
}
