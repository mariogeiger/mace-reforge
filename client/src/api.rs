use mace_reforge_shared::User;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

macro_rules! log {
    ($($t:tt)*) => {
        web_sys::console::log_1(&format!($($t)*).into())
    };
}

pub(crate) use log;

async fn fetch_json(
    url: &str,
    opts: &web_sys::RequestInit,
) -> Result<serde_json::Value, String> {
    let window = web_sys::window().unwrap();
    let resp: web_sys::Response = JsFuture::from(window.fetch_with_str_and_init(url, opts))
        .await
        .map_err(|e| format!("{e:?}"))?
        .dyn_into()
        .map_err(|e| format!("{e:?}"))?;
    let status = resp.status();
    let text = JsFuture::from(resp.text().map_err(|e| format!("{e:?}"))?)
        .await
        .map_err(|e| format!("{e:?}"))?
        .as_string()
        .unwrap_or_default();
    if status >= 400 {
        return Err(format!("HTTP {status}: {text}"));
    }
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

pub async fn api_get<T: serde::de::DeserializeOwned>(url: &str) -> Result<T, String> {
    let opts = web_sys::RequestInit::new();
    opts.set_method("GET");
    let json = fetch_json(url, &opts).await?;
    serde_json::from_value(json).map_err(|e| e.to_string())
}

pub async fn api_delete(url: &str) -> Result<(), String> {
    let opts = web_sys::RequestInit::new();
    opts.set_method("DELETE");
    let window = web_sys::window().unwrap();
    let resp: web_sys::Response = JsFuture::from(window.fetch_with_str_and_init(url, &opts))
        .await
        .map_err(|e| format!("{e:?}"))?
        .dyn_into()
        .map_err(|e| format!("{e:?}"))?;
    let status = resp.status();
    if status >= 400 {
        let text = JsFuture::from(resp.text().map_err(|e| format!("{e:?}"))?)
            .await
            .map_err(|e| format!("{e:?}"))?
            .as_string()
            .unwrap_or_default();
        return Err(format!("HTTP {status}: {text}"));
    }
    Ok(())
}

pub async fn api_post<T: serde::de::DeserializeOwned>(
    url: &str,
    body: &impl serde::Serialize,
) -> Result<T, String> {
    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    let headers = web_sys::Headers::new().unwrap();
    headers.set("Content-Type", "application/json").unwrap();
    opts.set_headers(&headers);
    opts.set_body(&JsValue::from_str(&serde_json::to_string(body).unwrap()));
    let json = fetch_json(url, &opts).await?;
    serde_json::from_value(json).map_err(|e| e.to_string())
}

fn storage() -> web_sys::Storage {
    web_sys::window()
        .unwrap()
        .local_storage()
        .unwrap()
        .unwrap()
}

pub fn load_local_user() -> Option<User> {
    let s = storage().get_item("user").ok()??;
    serde_json::from_str(&s).ok()
}

pub fn save_local_user(user: &User) {
    let json = serde_json::to_string(user).unwrap();
    storage().set_item("user", &json).ok();
}
