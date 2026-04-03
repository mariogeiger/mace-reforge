use leptos::prelude::*;
use mace_reforge_shared::*;

use crate::api::*;
use crate::Star;

#[component]
pub fn HomePage() -> impl IntoView {
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
