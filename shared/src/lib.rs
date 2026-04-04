use serde::{Deserialize, Serialize};

// ── Question types ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum QuestionKind {
    #[default]
    Closed,
    Open,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub topic_id: String,
    pub text: String,
    #[serde(default)]
    pub kind: QuestionKind,
    /// Closed question: fixed answer positions around a circle
    #[serde(default)]
    pub answers: Vec<String>,
    /// Closed question: user vote positions on the circle
    #[serde(default)]
    pub votes: Vec<Vote>,
    /// Open question: free-text answers
    #[serde(default)]
    pub open_answers: Vec<OpenAnswer>,
    /// Custom projection axes: (negative_text, positive_text)
    #[serde(default)]
    pub x_axis: Option<(String, String)>,
    #[serde(default)]
    pub y_axis: Option<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vote {
    pub user_name: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAnswer {
    pub user_name: String,
    pub text: String,
}

// ── Topics ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    pub id: String,
    pub title: String,
}

/// Topic with computed question count, returned by API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicWithCount {
    pub id: String,
    pub title: String,
    pub question_count: usize,
}

// ── Users ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Shape {
    Circle,
    Square,
    Triangle,
    Diamond,
    Star,
    Hexagon,
    Heart,
    Arrow,
    Lightning,
    Drop,
    Leaf,
    Cross,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub name: String,
    pub shape: Shape,
    pub color: String,
}

// ── API request/response types ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTopic {
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateQuestion {
    pub text: String,
    #[serde(default)]
    pub kind: QuestionKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddAnswer {
    pub text: String,
    pub index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditAnswer {
    pub index: usize,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteAnswer {
    pub index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastVote {
    pub user_name: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddOpenAnswer {
    pub user_name: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetAxes {
    pub x_axis: Option<(String, String)>,
    pub y_axis: Option<(String, String)>,
}

/// 2D positions for open question answers, returned by the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanePositions {
    pub points: Vec<PlanePoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanePoint {
    pub user_name: String,
    pub x: f64,
    pub y: f64,
}

// ── WebSocket messages ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsMsg {
    /// Ephemeral position update during drag (not persisted).
    VoteMoved { user_name: String, x: f64, y: f64 },
    /// Full question state after a persistent change.
    QuestionUpdated { question: Question },
}
