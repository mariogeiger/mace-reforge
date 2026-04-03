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
    }
}
