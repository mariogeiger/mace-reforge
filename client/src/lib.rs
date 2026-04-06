mod api;
mod closed_question;
mod home;
mod open_question;
mod question;
mod shapes;
mod topic;
mod user_badge;

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

use api::load_local_user;
use home::HomePage;
use question::QuestionPage;
use topic::TopicPage;
use user_badge::UserBadge;

// ── Routing ─────────────────────────────────────────────────────────

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

// ── Star icon ───────────────────────────────────────────────────────

const STAR_PATH: &str = "M 211.88,0 C 211.88,172.25 173.19,210.94 0.94,210.94 173.19,210.94 211.88,249.63 211.88,421.88 211.88,249.63 250.57,210.94 422.82,210.94 250.57,210.94 211.88,172.25 211.88,0 Z";

#[component]
pub fn Star(class_name: &'static str) -> impl IntoView {
    view! {
        <div class=class_name>
            <svg viewBox="0 0 423 422" xmlns="http://www.w3.org/2000/svg">
                <path d=STAR_PATH fill="currentColor"/>
            </svg>
        </div>
    }
}

// ── Entry ───────────────────────────────────────────────────────────

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let (route, set_route) = signal(parse_hash());
    let (current_user, set_current_user) = signal(load_local_user());

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
            <UserBadge user=current_user set_user=set_current_user/>
        </header>
        <main>
            {move || match route.get() {
                Route::Home => view! { <HomePage/> }.into_any(),
                Route::Topic(id) => view! { <TopicPage topic_id=id/> }.into_any(),
                Route::Question(tid, qid) => view! { <QuestionPage topic_id=tid question_id=qid current_user=current_user/> }.into_any(),
            }}
        </main>
        <footer class="site-footer">
            "Created by Mario Geiger as a personal initiative, independent of and unaffiliated with NVIDIA."
            <a href="https://github.com/mariogeiger/mace-reforge" target="_blank" rel="noopener" class="github-link">
                <svg viewBox="0 0 16 16" width="20" height="20" fill="currentColor">
                    <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27s1.36.09 2 .27c1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0016 8c0-4.42-3.58-8-8-8z"/>
                </svg>
            </a>
        </footer>
    }
}
