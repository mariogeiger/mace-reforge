use leptos::prelude::*;
use mace_reforge_shared::*;

use crate::api::*;
use crate::closed_question::ClosedQuestionPage;
use crate::open_question::OpenQuestionPage;

#[component]
pub fn QuestionPage(
    topic_id: String,
    question_id: String,
    current_user: ReadSignal<Option<User>>,
) -> impl IntoView {
    let (question, set_question) = signal(Option::<Question>::None);
    let (kind, set_kind) = signal(Option::<QuestionKind>::None);

    let tid = topic_id.clone();
    let qid = question_id.clone();
    Effect::new(move || {
        let tid = tid.clone();
        let qid = qid.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(q) =
                api_get::<Question>(&format!("/api/topics/{tid}/questions/{qid}")).await
            {
                set_kind.set(Some(q.kind.clone()));
                set_question.set(Some(q));
            }
        });
    });

    let tid2 = topic_id.clone();
    let qid2 = question_id.clone();

    // Dispatch based on `kind` (set once), not `question` (updated on every submit)
    view! {
        {move || {
            let Some(k) = kind.get() else {
                return view! { <div class="page">"Loading..."</div> }.into_any();
            };
            match k {
                QuestionKind::Closed => view! {
                    <ClosedQuestionPage
                        topic_id=tid2.clone()
                        question_id=qid2.clone()
                        question=question
                        set_question=set_question
                        current_user=current_user
                    />
                }.into_any(),
                QuestionKind::Open => view! {
                    <OpenQuestionPage
                        topic_id=tid2.clone()
                        question_id=qid2.clone()
                        question=question
                        set_question=set_question
                        current_user=current_user
                    />
                }.into_any(),
            }
        }}
    }
}
