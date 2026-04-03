use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub topic_id: String,
    pub text: String,
    pub answers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTopic {
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateQuestion {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddAnswer {
    pub text: String,
    pub index: usize,
}

/// Topic with computed question count, returned by API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicWithCount {
    pub id: String,
    pub title: String,
    pub question_count: usize,
}
