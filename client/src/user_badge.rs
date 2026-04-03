use leptos::prelude::*;
use mace_reforge_shared::*;

use crate::api::*;
use crate::shapes::*;

#[component]
pub fn UserBadge(
    user: ReadSignal<Option<User>>,
    set_user: WriteSignal<Option<User>>,
) -> impl IntoView {
    let (editing, set_editing) = signal(false);
    let (name_input, set_name_input) = signal(String::new());
    let (selected_shape, set_selected_shape) = signal(Shape::Circle);
    let (selected_color, set_selected_color) = signal(PALETTE[0].to_string());
    let (known_users, set_known_users) = signal(Vec::<User>::new());
    let (suggestions, set_suggestions) = signal(Vec::<User>::new());

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
