//! AI 对话：调用 DeepSeek（OpenAI 兼容）接口，让模型按需读取当前文件夹中的文件。
//!
//! 设计要点：
//! - API Key 优先使用用户在设置中保存的密钥，其次读取环境变量 `DEEPSEEK_APIKEY`；
//! - 保存的密钥用 Windows DPAPI 加密（见 `crate::secret`），配置写入
//!   `%APPDATA%\wmfold\ai.json`，不进入 `settings.json`；
//! - 模型通过 function calling 调用 `list_dir` / `read_file` / `search_files`
//!   / `file_info` 四个只读工具，后端负责执行并把结果回传给模型；
//! - 所有路径都被限制在当前文件夹（含子文件夹）内，避免模型读取无关文件；
//! - 每次工具调用通过 `ai-tool` 事件推送给界面，用户能看到“正在读取哪个文件”。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use wmfold_core::{chem, fs};

use crate::secret;

const DEFAULT_MODEL: &str = "deepseek-flash";
const DEFAULT_BASE_URL: &str = "https://api.deepseek.com";
const DEFAULT_REASONING_EFFORT: &str = "high";
const CONFIG_FILE: &str = "ai.json";
/// 单次对话最多允许的“模型 → 工具 → 模型”轮数。
const MAX_TOOL_ROUNDS: usize = 12;
/// 单次读取文件的最大字节数（默认 96 KB，上限 256 KB）。
const DEFAULT_READ_BYTES: usize = 96 * 1024;
const MAX_READ_BYTES: usize = 256 * 1024;
/// 单条消息 / 整个对话的字符数上限（防止把超大文件内容塞进上下文）。
const MAX_MESSAGE_CHARS: usize = 48_000;
const MAX_TOTAL_CHARS: usize = 240_000;

// ---- 配置 ----

/// 落盘配置。`api_key` 保存的是加密后的文本（`dpapi:...` / `plain:...`）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct StoredAiConfig {
    #[serde(default)]
    api_key: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    base_url: String,
    /// 是否开启思考模式。旧配置没有该字段时按关闭处理（更快且不需要回传思维链）。
    #[serde(default)]
    thinking: bool,
    /// low / high / max，仅在思考模式下生效。
    #[serde(default)]
    reasoning_effort: String,
}

/// 返回给前端的配置视图。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfigView {
    /// 已保存的 API Key（解密后；仅用于设置页编辑）
    pub api_key: String,
    /// 环境变量中是否配置了 Key
    pub has_env_key: bool,
    /// 环境变量 Key 的末 4 位（不泄露完整 Key）
    pub env_key_preview: String,
    /// 实际生效的来源：saved / env / none
    pub key_source: String,
    /// 生效 Key 的末 4 位提示
    pub key_preview: String,
    pub model: String,
    pub base_url: String,
    /// 思考模式开关
    pub thinking: bool,
    /// low / high / max
    pub reasoning_effort: String,
}

fn config_path() -> PathBuf {
    crate::settings::config_dir().join(CONFIG_FILE)
}

fn load_stored() -> StoredAiConfig {
    std::fs::read_to_string(config_path())
        .ok()
        .and_then(|text| serde_json::from_str::<StoredAiConfig>(&text).ok())
        .unwrap_or_default()
}

fn normalize_model(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return DEFAULT_MODEL.to_string();
    }
    // 官网当前模型为 deepseek-flash / deepseek-v4-pro；把旧模型名迁移到新名字。
    match value.to_ascii_lowercase().as_str() {
        "deepseek-chat" | "deepseek-reasoner" | "deepseek-r1" | "deepseek-v4-flash"
        | "deepseek-v4-flash-vision-exp" => DEFAULT_MODEL.to_string(),
        _ => value.to_string(),
    }
}

fn normalize_base_url(value: &str) -> String {
    let value = value.trim().trim_end_matches('/');
    if value.is_empty() || !(value.starts_with("https://") || value.starts_with("http://")) {
        DEFAULT_BASE_URL.to_string()
    } else if value.eq_ignore_ascii_case("https://api.deepseek.com/v1") {
        // 官网 OpenAI 兼容地址不再需要 /v1；旧配置自动迁移。
        DEFAULT_BASE_URL.to_string()
    } else {
        value.to_string()
    }
}

fn normalize_reasoning_effort(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "low" | "minimal" => "low".to_string(),
        "max" | "ultra" => "max".to_string(),
        "high" | "medium" | "xhigh" | "" => DEFAULT_REASONING_EFFORT.to_string(),
        _ => DEFAULT_REASONING_EFFORT.to_string(),
    }
}

/// 旧版 `deepseek-reasoner` 配置默认开启思考模式，避免迁移后丢掉推理能力。
fn stored_thinking(stored: &StoredAiConfig) -> bool {
    stored.thinking
        || matches!(
            stored.model.trim().to_ascii_lowercase().as_str(),
            "deepseek-reasoner" | "deepseek-r1"
        )
}

fn chat_endpoint(base: &str) -> String {
    let base = base.trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        base.to_string()
    } else {
        format!("{base}/chat/completions")
    }
}

fn models_endpoint(base: &str) -> String {
    let mut base = base.trim_end_matches('/');
    if let Some(stripped) = base.strip_suffix("/chat/completions") {
        base = stripped;
    }
    format!("{base}/models")
}

fn view(stored: &StoredAiConfig) -> AiConfigView {
    let saved = secret::unprotect(&stored.api_key).trim().to_string();
    let env = std::env::var("DEEPSEEK_APIKEY")
        .unwrap_or_default()
        .trim()
        .to_string();
    let (source, preview) = if !saved.is_empty() {
        ("saved", secret::preview(&saved))
    } else if !env.is_empty() {
        ("env", secret::preview(&env))
    } else {
        ("none", String::new())
    };
    AiConfigView {
        api_key: saved,
        has_env_key: !env.is_empty(),
        env_key_preview: secret::preview(&env),
        key_source: source.to_string(),
        key_preview: preview,
        model: normalize_model(&stored.model),
        base_url: normalize_base_url(&stored.base_url),
        thinking: stored_thinking(stored),
        reasoning_effort: normalize_reasoning_effort(&stored.reasoning_effort),
    }
}

struct ResolvedAiConfig {
    key: String,
    model: String,
    base_url: String,
    thinking: bool,
    reasoning_effort: String,
}

/// 取出生效的 Key / 模型 / Base URL / 思考模式。保存的 Key 优先于环境变量。
fn resolve_config(stored: &StoredAiConfig) -> Result<ResolvedAiConfig, String> {
    let saved = secret::unprotect(&stored.api_key).trim().to_string();
    let env = std::env::var("DEEPSEEK_APIKEY")
        .unwrap_or_default()
        .trim()
        .to_string();
    let key = if !saved.is_empty() { saved } else { env };
    if key.is_empty() {
        return Err(
            "尚未配置 DeepSeek API Key：请到 设置 → AI 对话 填写，或设置环境变量 DEEPSEEK_APIKEY。"
                .to_string(),
        );
    }
    Ok(ResolvedAiConfig {
        key,
        model: normalize_model(&stored.model),
        base_url: normalize_base_url(&stored.base_url),
        thinking: stored_thinking(stored),
        reasoning_effort: normalize_reasoning_effort(&stored.reasoning_effort),
    })
}

// ---- 命令 ----

/// 读取 AI 配置（含 Key 来源提示）。
#[tauri::command]
pub fn ai_config() -> AiConfigView {
    view(&load_stored())
}

/// 保存 AI 配置。`api_key` 留空表示清除已保存的 Key（之后回退到环境变量）。
#[tauri::command(async)]
pub fn ai_save_config(
    api_key: String,
    model: String,
    base_url: String,
    thinking: Option<bool>,
    reasoning_effort: Option<String>,
) -> Result<AiConfigView, String> {
    let stored = StoredAiConfig {
        api_key: secret::protect(api_key.trim()),
        model: normalize_model(&model),
        base_url: normalize_base_url(&base_url),
        thinking: thinking.unwrap_or(false),
        reasoning_effort: normalize_reasoning_effort(
            reasoning_effort.as_deref().unwrap_or(DEFAULT_REASONING_EFFORT),
        ),
    };
    let dir = crate::settings::config_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(&stored).map_err(|e| e.to_string())?;
    std::fs::write(config_path(), text).map_err(|e| e.to_string())?;
    Ok(view(&stored))
}

/// 测试 DeepSeek 连接：读取 `/models`，用于确认 Key 与 Base URL 是否可用。
///
/// 注意：`reqwest::blocking` 不能直接在 Tokio 工作线程里创建（会 panic：
/// “Cannot drop a runtime in a context where blocking is not allowed”），
/// 所以必须放进专用阻塞线程池。
#[tauri::command]
pub async fn ai_test() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(ai_test_blocking)
        .await
        .map_err(|err| format!("AI 请求线程异常：{err}"))?
}

fn ai_test_blocking() -> Result<String, String> {
    let stored = load_stored();
    let config = resolve_config(&stored)?;
    let client = http_client()?;
    let resp = client
        .get(models_endpoint(&config.base_url))
        .bearer_auth(&config.key)
        .send()
        .map_err(network_error)?;
    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(api_error(status, &text));
    }
    let json: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let ids: Vec<String> = json["data"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|item| item["id"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    if ids.is_empty() {
        Ok(format!(
            "连接成功（当前模型：{}，思考模式：{}）",
            config.model,
            if config.thinking { "开启" } else { "关闭" }
        ))
    } else {
        Ok(format!(
            "连接成功，当前模型：{}（{}），可用模型：{}",
            config.model,
            if config.thinking { "思考模式" } else { "非思考模式" },
            ids.join("、")
        ))
    }
}

// ---- 对话 ----

/// 前端传入的一条对话消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatMessage {
    pub role: String,
    pub content: String,
    /// 思考模式下上一轮 assistant 的思维链；工具调用请求必须原样回传。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}

/// 工具返回的一个可点击文件 / 文件夹结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiToolResult {
    /// 绝对路径，用于文件列表定位。
    pub path: String,
    pub name: String,
    /// 相对于当前根目录的显示路径。
    pub rel: String,
    pub is_dir: bool,
}

/// 一次工具调用（用于界面展示“读取了哪些文件”）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiToolStep {
    pub id: String,
    pub name: String,
    /// 人类可读的动作描述，例如「读取 src/App.vue」
    pub label: String,
    /// running / ok / error
    pub status: String,
    pub summary: String,
    /// 工具实际命中的文件 / 文件夹，供界面点击定位。
    pub results: Vec<AiToolResult>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiReply {
    pub content: String,
    pub steps: Vec<AiToolStep>,
    pub model: String,
    /// 思考模式下本轮最终回答的思维链，前端下一轮需要原样带回。
    pub reasoning_content: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AiToolEvent {
    request_id: u64,
    step: AiToolStep,
}

/// 与 DeepSeek 对话。后端自动完成多轮工具调用，返回最终回答与工具调用轨迹。
#[tauri::command]
pub async fn ai_chat(
    app: AppHandle,
    request_id: u64,
    root: String,
    messages: Vec<AiChatMessage>,
    selected: Vec<String>,
    recursive: bool,
) -> Result<AiReply, String> {
    tauri::async_runtime::spawn_blocking(move || {
        ai_chat_blocking(app, request_id, root, messages, selected, recursive)
    })
    .await
    .map_err(|err| format!("AI 对话线程异常：{err}"))?
}

fn ai_chat_blocking(
    app: AppHandle,
    request_id: u64,
    root: String,
    messages: Vec<AiChatMessage>,
    selected: Vec<String>,
    recursive: bool,
) -> Result<AiReply, String> {
    let stored = load_stored();
    let config = resolve_config(&stored)?;

    let root_raw = root.trim();
    if root_raw.is_empty() || root_raw == "::ThisPC" {
        return Err("请先进入一个文件夹，再使用 AI 对话。".to_string());
    }
    let root = Path::new(root_raw)
        .canonicalize()
        .map_err(|_| format!("目录不存在：{root_raw}"))?;
    if !root.is_dir() {
        return Err(format!("不是文件夹：{root_raw}"));
    }

    let history = sanitize_messages(messages)?;
    if history.is_empty() {
        return Err("消息不能为空。".to_string());
    }

    let selected_rel: Vec<String> = selected
        .iter()
        .take(30)
        .filter_map(|path| resolve_path(&root, path, recursive).ok())
        .map(|path| rel_display(&root, &path))
        .collect();

    let client = http_client()?;
    let endpoint = chat_endpoint(&config.base_url);
    chat_loop(
        &client,
        &endpoint,
        &config.key,
        &config.model,
        config.thinking,
        &config.reasoning_effort,
        recursive,
        &root,
        &history,
        &selected_rel,
        |step| emit_step(&app, request_id, step),
    )
}

/// 执行“模型 → 工具 → 模型”的多轮循环，直到得到最终回答。
#[allow(clippy::too_many_arguments)]
fn chat_loop(
    client: &Client,
    endpoint: &str,
    key: &str,
    model: &str,
    thinking: bool,
    reasoning_effort: &str,
    recursive: bool,
    root: &Path,
    history: &[AiChatMessage],
    selected_rel: &[String],
    mut on_step: impl FnMut(&AiToolStep),
) -> Result<AiReply, String> {
    // 官网要求：带 tools 的思考请求必须完整回传历史 assistant 的 reasoning_content。
    // 如果历史来自旧版本（没有思维链），本轮自动退回非思考模式，避免 400。
    let thinking = thinking
        && history
            .iter()
            .filter(|message| message.role == "assistant")
            .all(|message| {
                message
                    .reasoning_content
                    .as_deref()
                    .is_some_and(|reasoning| !reasoning.trim().is_empty())
            });

    let mut wire: Vec<Value> = vec![json!({
        "role": "system",
        "content": system_prompt(root, selected_rel, recursive),
    })];
    for message in history {
        let mut item = json!({ "role": message.role, "content": message.content });
        if thinking && message.role == "assistant" {
            item["reasoning_content"] = json!(message.reasoning_content.as_deref().unwrap_or(""));
        }
        wire.push(item);
    }

    let mut steps: Vec<AiToolStep> = Vec::new();
    let mut seen_calls: std::collections::HashSet<String> = std::collections::HashSet::new();

    for _round in 0..MAX_TOOL_ROUNDS {
        let json = post_chat(
            client,
            endpoint,
            key,
            model,
            &wire,
            thinking,
            reasoning_effort,
            "auto",
        )?;
        let message = &json["choices"][0]["message"];
        let content = message["content"].as_str().unwrap_or("").trim().to_string();
        let reasoning = message["reasoning_content"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();
        let tool_calls = message["tool_calls"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        if tool_calls.is_empty() {
            if content.is_empty() {
                return Err("DeepSeek 返回了空回复，请重试。".to_string());
            }
            return Ok(AiReply {
                content,
                steps,
                model: model.to_string(),
                reasoning_content: if thinking && !reasoning.is_empty() {
                    Some(reasoning)
                } else {
                    None
                },
            });
        }

        let mut assistant = json!({
            "role": "assistant",
            "content": if content.is_empty() { Value::Null } else { json!(content) },
        });
        if thinking {
            // 即使本轮为空，也必须带 reasoning_content 字段，否则下一轮会 400。
            assistant["reasoning_content"] = json!(reasoning);
        }
        assistant["tool_calls"] = Value::Array(tool_calls.clone());
        wire.push(assistant);

        for call in &tool_calls {
            let id = call["id"].as_str().unwrap_or("call").to_string();
            let name = call["function"]["name"].as_str().unwrap_or("").to_string();
            let args_text = call["function"]["arguments"].as_str().unwrap_or("{}");
            let args: Value = serde_json::from_str(args_text).unwrap_or_else(|_| json!({}));

            let fingerprint = format!("{name}\u{1f}{args_text}");
            let duplicate = !seen_calls.insert(fingerprint);

            let mut step = AiToolStep {
                id: id.clone(),
                name: name.clone(),
                label: tool_label(&name, &args),
                status: "running".to_string(),
                summary: String::new(),
                results: Vec::new(),
            };
            if duplicate {
                step.status = "error".to_string();
                step.summary = "重复调用，已跳过".to_string();
                on_step(&step);
                steps.push(step);
                wire.push(json!({
                    "role": "tool",
                    "tool_call_id": id,
                    "content": "该工具调用与之前完全相同，结果已在上文，已跳过。请不要重复调用，直接根据已有结果回答。",
                }));
                continue;
            }
            on_step(&step);

            let (output, summary, status, results) = match run_tool(root, &name, &args, recursive) {
                Ok(result) => (result.output, result.summary, "ok", result.results),
                Err(err) => (
                    format!("工具调用失败：{err}"),
                    err,
                    "error",
                    Vec::new(),
                ),
            };
            step.status = status.to_string();
            step.summary = summary;
            step.results = results;
            steps.push(step.clone());
            on_step(&step);

            wire.push(json!({
                "role": "tool",
                "tool_call_id": id,
                "content": output,
            }));
        }
    }

    // 轮数用完：再请求一次，但强制不允许工具调用，让模型基于已有结果收尾。
    wire.push(json!({
        "role": "user",
        "content": "工具调用轮数已达上限。请不要再调用任何工具，直接根据上面已经获得的工具结果回答用户的问题；如果信息不足，请明确说明还需要什么，不要编造。",
    }));
    let json = post_chat(
        client,
        endpoint,
        key,
        model,
        &wire,
        thinking,
        reasoning_effort,
        "none",
    )?;
    let message = &json["choices"][0]["message"];
    let content = message["content"].as_str().unwrap_or("").trim().to_string();
    if content.is_empty() {
        return Err(format!(
            "工具调用超过 {MAX_TOOL_ROUNDS} 轮，且模型没有生成最终回答。请把问题拆小一些再试。"
        ));
    }
    let reasoning = message["reasoning_content"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();
    Ok(AiReply {
        content,
        steps,
        model: model.to_string(),
        reasoning_content: if thinking && !reasoning.is_empty() {
            Some(reasoning)
        } else {
            None
        },
    })
}

/// 发送一次 Chat Completions 请求，返回解析后的 JSON。
#[allow(clippy::too_many_arguments)]
fn post_chat(
    client: &Client,
    endpoint: &str,
    key: &str,
    model: &str,
    wire: &[Value],
    thinking: bool,
    reasoning_effort: &str,
    tool_choice: &str,
) -> Result<Value, String> {
    let mut body = json!({
        "model": model,
        "messages": wire,
        "tools": tool_schemas(),
        "tool_choice": tool_choice,
        "stream": false,
        "thinking": { "type": if thinking { "enabled" } else { "disabled" } },
    });
    if thinking {
        // 思考模式下 temperature 不生效，改为传递思考强度。
        body["reasoning_effort"] = json!(reasoning_effort);
    } else {
        body["temperature"] = json!(0.2);
    }
    let resp = client
        .post(endpoint)
        .bearer_auth(key)
        .json(&body)
        .send()
        .map_err(network_error)?;
    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(api_error(status, &text));
    }
    serde_json::from_str(&text).map_err(|e| format!("解析 DeepSeek 响应失败：{e}"))
}

fn sanitize_messages(messages: Vec<AiChatMessage>) -> Result<Vec<AiChatMessage>, String> {
    let mut out = Vec::new();
    let mut total = 0usize;
    for message in messages {
        let role = message.role.trim();
        if role != "user" && role != "assistant" {
            continue;
        }
        let content = message.content.trim();
        if content.is_empty() {
            continue;
        }
        if content.chars().count() > MAX_MESSAGE_CHARS {
            return Err("单条消息过长，请缩短后重试。".to_string());
        }
        let reasoning = message
            .reasoning_content
            .unwrap_or_default()
            .trim()
            .to_string();
        total += content.chars().count();
        if role == "assistant" {
            total += reasoning.chars().count();
        }
        if total > MAX_TOTAL_CHARS {
            return Err("对话内容过长，请清空对话后重试。".to_string());
        }
        out.push(AiChatMessage {
            role: role.to_string(),
            content: content.to_string(),
            reasoning_content: if role == "assistant" && !reasoning.is_empty() {
                Some(reasoning)
            } else {
                None
            },
        });
    }
    Ok(out)
}

fn emit_step(app: &AppHandle, request_id: u64, step: &AiToolStep) {
    let _ = app.emit(
        "ai-tool",
        AiToolEvent {
            request_id,
            step: step.clone(),
        },
    );
}

// ---- HTTP / TLS ----

fn ensure_crypto_provider() {
    // reqwest 使用 rustls-no-provider，需要显式安装 ring 提供的默认加密实现。
    // tauri-plugin-updater 也会安装；这里保证在检查更新之前使用 AI 也能工作。
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
}

fn http_client() -> Result<Client, String> {
    ensure_crypto_provider();
    Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| format!("初始化网络客户端失败：{e}"))
}

fn network_error(err: reqwest::Error) -> String {
    if err.is_timeout() {
        "请求 DeepSeek 超时，请检查网络后重试。".to_string()
    } else if err.is_connect() {
        format!("无法连接 DeepSeek：{err}。请检查网络、代理或 Base URL。")
    } else {
        format!("请求 DeepSeek 失败：{err}")
    }
}

fn api_error(status: StatusCode, body: &str) -> String {
    let detail = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|json| {
            json["error"]["message"]
                .as_str()
                .map(|s| s.to_string())
                .or_else(|| json["message"].as_str().map(|s| s.to_string()))
        })
        .unwrap_or_else(|| body.chars().take(300).collect());
    match status.as_u16() {
        401 => format!("DeepSeek API Key 无效或已过期（401）：{detail}"),
        402 => format!("DeepSeek 账户余额不足（402）：{detail}"),
        429 => format!("DeepSeek 请求过于频繁（429），请稍后再试：{detail}"),
        code => format!("DeepSeek API 错误（{code}）：{detail}"),
    }
}

// ---- 工具 ----

fn tool_schemas() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "list_dir",
                "description": "列出当前文件夹内某个目录的直接子目录和文件（不递归）。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "目录路径，相对当前文件夹，例如 \".\" 或 \"sub\"" }
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "读取文本文件内容。文件较大时只返回前 maxBytes 字节（默认 96KB，最大 256KB）。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "文件路径，相对当前文件夹" },
                        "maxBytes": { "type": "integer", "description": "最多读取的字节数，默认 98304" }
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "search_files",
                "description": "递归搜索当前文件夹下的文件和内容。默认按文件名匹配；content=true 时搜索文件内容（仅文本文件，跳过 512KB 以上文件，最多检查 2000 个文件）。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "要搜索的文本（不区分大小写）" },
                        "path": { "type": "string", "description": "搜索起始目录，默认当前文件夹" },
                        "content": { "type": "boolean", "description": "是否搜索文件内容，默认 false" },
                        "maxResults": { "type": "integer", "description": "最多返回多少条结果，默认 30" }
                    },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "file_info",
                "description": "查看文件或文件夹的基本信息；化学文件还会返回化学式、原子数、电荷、多重度、基组函数数、分子量、能量和作业关键词。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "文件或文件夹路径，相对当前文件夹" }
                    },
                    "required": ["path"]
                }
            }
        }
    ])
}

fn system_prompt(root: &Path, selected: &[String], recursive: bool) -> String {
    let selected_text = if selected.is_empty() {
        "（无）".to_string()
    } else {
        selected.join("、")
    };
    let scope = if recursive {
        "当前文件夹及其所有子文件夹（扫描模式）"
    } else {
        "当前文件夹的第一层，不能进入子文件夹（普通模式）"
    };
    let search_desc = if recursive {
        "search_files：按文件名或文件内容递归搜索；"
    } else {
        "search_files：按文件名或文件内容搜索当前文件夹第一层；"
    };
    let nested_hint = if recursive {
        "扫描模式下可以访问子文件夹。"
    } else {
        "普通模式下不要进入子文件夹；如果用户需要子文件夹内容，提示他先点击「扫描」。"
    };
    format!(
        r#"你是 wmfold（一个 Windows 风格的化学文件资源管理器）内置的 AI 文件助手。

当前工作目录（唯一允许访问的根目录）：{root}
当前访问范围：{scope}
当前选中的文件：{selected}

你可以调用以下工具查看真实文件后再回答：
- list_dir：列出目录内容；
- read_file：读取文本文件（超出大小会截断）；
- {search_desc}
- file_info：查看文件信息，化学文件会返回化学式 / 原子数 / 能量 / 作业关键词等。

规则：
1. 必须先调用工具确认文件真实存在与内容，不要凭空猜测或编造文件名、数据。
2. 只能访问 {scope} 中的内容，路径使用相对路径（例如 "."、"sub/a.gjf"）。工具会拒绝越界路径。{nested_hint}
3. 文件很大时先用 search_files 或分段 read_file 定位关键内容，不要一次读取整个大文件；内容搜索会跳过 512KB 以上的文件，大 log 请按文件名搜索或直接 file_info。
4. 回答使用中文，条理清晰；引用文件时给出相对路径（如 sub/a.gjf），引用文件夹时以 / 结尾（如 sub/），涉及多个文件时用列表汇总。
5. 如果用户的问题与当前文件夹无关，可以简短说明你能帮他查看这里哪些文件。
6. 如果工具返回错误（路径不存在、二进制文件等），向用户说明并给出可行的下一步。
7. 不要重复调用完全相同的工具；某个搜索返回 0 条时，换关键词/路径或直接根据已有结果回答。已经读到足够信息后不要再逐个文件调用 file_info，工具调用轮数有限。"#,
        root = display_root(root),
        scope = scope,
        selected = selected_text,
        search_desc = search_desc,
        nested_hint = nested_hint,
    )
}

struct ToolResult {
    output: String,
    summary: String,
    results: Vec<AiToolResult>,
}

fn run_tool(root: &Path, name: &str, args: &Value, recursive: bool) -> Result<ToolResult, String> {
    match name {
        "list_dir" => tool_list_dir(root, args, recursive),
        "read_file" => tool_read_file(root, args, recursive),
        "search_files" => tool_search_files(root, args, recursive),
        "file_info" => tool_file_info(root, args, recursive),
        other => Err(format!("未知工具：{other}")),
    }
}

fn display_abs_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        text.to_string()
    }
}

fn tool_result_item(root: &Path, path: &Path, name: &str, is_dir: bool) -> AiToolResult {
    AiToolResult {
        path: display_abs_path(path),
        name: name.to_string(),
        rel: rel_display(root, path),
        is_dir,
    }
}

fn tool_label(name: &str, args: &Value) -> String {
    let path = args["path"].as_str().unwrap_or(".").trim();
    let path = if path.is_empty() { "." } else { path };
    match name {
        "list_dir" => format!("列出 {path}"),
        "read_file" => format!("读取 {path}"),
        "file_info" => format!("查看 {path}"),
        "search_files" => {
            let query = args["query"].as_str().unwrap_or("").trim();
            let content = args["content"].as_bool().unwrap_or(false);
            if content {
                format!("搜索文件内容「{query}」")
            } else {
                format!("搜索文件名「{query}」")
            }
        }
        other => format!("调用 {other}"),
    }
}

fn tool_list_dir(root: &Path, args: &Value, recursive: bool) -> Result<ToolResult, String> {
    let path = resolve_path(root, args["path"].as_str().unwrap_or("."), recursive)?;
    if !recursive && path != root {
        return Err("普通模式只能列出当前文件夹；请先点击「扫描」查看子文件夹。".to_string());
    }
    if !path.is_dir() {
        return Err(format!("{} 不是文件夹", rel_display(root, &path)));
    }
    let mut entries = fs::list_dir(&path.to_string_lossy())?;
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    let total = entries.len();
    let mut out = format!(
        "目录：{}\n共 {} 个条目：\n",
        rel_display(root, &path),
        total
    );
    for entry in entries.iter().take(400) {
        if entry.is_dir {
            out.push_str(&format!("[目录] {}/\n", entry.name));
        } else {
            let kind = if entry.is_chem {
                format!("，{} 类型", entry.kind)
            } else {
                String::new()
            };
            out.push_str(&format!(
                "[文件] {}（{}字节{}）\n",
                entry.name,
                entry.size,
                kind
            ));
        }
    }
    if total > 400 {
        out.push_str("…（其余条目已省略，可用 search_files 查找）\n");
    }
    let results = entries
        .iter()
        .take(80)
        .map(|entry| {
            tool_result_item(
                root,
                Path::new(&entry.path),
                &entry.name,
                entry.is_dir,
            )
        })
        .collect();
    Ok(ToolResult {
        output: out,
        summary: format!("{total} 个条目"),
        results,
    })
}

fn tool_read_file(root: &Path, args: &Value, recursive: bool) -> Result<ToolResult, String> {
    let path = resolve_path(root, args["path"].as_str().unwrap_or(""), recursive)?;
    if path.is_dir() {
        return Err(format!(
            "{} 是文件夹，请用 list_dir",
            rel_display(root, &path)
        ));
    }
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    let max_bytes = args["maxBytes"]
        .as_u64()
        .unwrap_or(DEFAULT_READ_BYTES as u64)
        .clamp(1024, MAX_READ_BYTES as u64) as usize;
    let mut file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
    let mut buffer = Vec::with_capacity(max_bytes.min(meta.len() as usize) + 1);
    file.take(max_bytes as u64 + 1)
        .read_to_end(&mut buffer)
        .map_err(|e| e.to_string())?;
    let truncated = buffer.len() > max_bytes;
    buffer.truncate(max_bytes);

    if buffer.iter().take(4096).any(|byte| *byte == 0) {
        return Err(format!(
            "{}（{}）看起来是二进制文件，无法按文本读取",
            rel_display(root, &path),
            human_size(meta.len())
        ));
    }

    let text = String::from_utf8_lossy(&buffer);
    let lines = text.lines().count();
    let mut out = format!(
        "文件：{}\n大小：{}（{} 字节）\n行数：{}\n----\n",
        rel_display(root, &path),
        human_size(meta.len()),
        meta.len(),
        lines
    );
    out.push_str(&text);
    if truncated {
        out.push_str(&format!(
            "\n…（文件超过 {}KB，已截断；可指定 maxBytes 或分段读取）",
            max_bytes / 1024
        ));
    }
    Ok(ToolResult {
        output: out,
        summary: format!("{} 行 / {}", lines, human_size(meta.len())),
        results: vec![tool_result_item(
            root,
            &path,
            path.file_name().and_then(|name| name.to_str()).unwrap_or(""),
            false,
        )],
    })
}

fn tool_search_files(root: &Path, args: &Value, recursive: bool) -> Result<ToolResult, String> {
    let base = resolve_path(root, args["path"].as_str().unwrap_or("."), recursive)?;
    if !recursive && base != root {
        return Err("普通模式只能搜索当前文件夹第一层；请先点击「扫描」搜索子文件夹。".to_string());
    }
    let query = args["query"].as_str().unwrap_or("").trim().to_string();
    if query.is_empty() {
        return Err("搜索内容不能为空".to_string());
    }
    let content_mode = args["content"].as_bool().unwrap_or(false);
    let limit = args["maxResults"]
        .as_u64()
        .unwrap_or(30)
        .clamp(1, 100) as usize;
    let needle = query.to_lowercase();

    let files = if recursive {
        fs::scan_recursive(&base.to_string_lossy(), false, 50_000, &|| false, |_| {})?.0
    } else {
        fs::list_dir(&base.to_string_lossy())?
            .into_iter()
            .filter(|entry| !entry.is_dir)
            .collect()
    };
    let mut hits: Vec<String> = Vec::new();
    let mut results: Vec<AiToolResult> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut checked = 0usize;

    for file in files {
        let rel = rel_display(root, Path::new(&file.path));
        if !content_mode {
            if file.name.to_lowercase().contains(&needle) {
                hits.push(format!("{rel}（{}）", human_size(file.size)));
                if seen.insert(file.path.clone()) {
                    results.push(tool_result_item(
                        root,
                        Path::new(&file.path),
                        &file.name,
                        file.is_dir,
                    ));
                }
            }
        } else {
            if checked >= 2000 {
                break;
            }
            // 只搜索较小的文本文件，避免把大日志/二进制读入内存
            if file.size > 512 * 1024 {
                continue;
            }
            checked += 1;
            let text = match std::fs::read(&file.path) {
                Ok(bytes) => {
                    if bytes.iter().take(2048).any(|byte| *byte == 0) {
                        continue;
                    }
                    String::from_utf8_lossy(&bytes).to_string()
                }
                Err(_) => continue,
            };
            for (index, line) in text.lines().enumerate() {
                if line.to_lowercase().contains(&needle) {
                    hits.push(format!(
                        "{rel}:{}: {}",
                        index + 1,
                        truncate_line(line.trim(), 180)
                    ));
                    if seen.insert(file.path.clone()) {
                        results.push(tool_result_item(
                            root,
                            Path::new(&file.path),
                            &file.name,
                            file.is_dir,
                        ));
                    }
                    if hits.len() >= limit {
                        break;
                    }
                }
            }
        }
        if hits.len() >= limit {
            break;
        }
    }

    let mode = if content_mode { "文件内容" } else { "文件名" };
    let mut out = format!(
        "搜索：{}（按{mode}，起始目录 {}）\n共 {} 条结果：\n",
        query,
        rel_display(root, &base),
        hits.len()
    );
    if hits.is_empty() {
        out.push_str("（没有匹配项）\n");
    } else {
        for hit in &hits {
            out.push_str(hit);
            out.push('\n');
        }
    }
    Ok(ToolResult {
        output: out,
        summary: format!("{} 个匹配", hits.len()),
        results,
    })
}

fn tool_file_info(root: &Path, args: &Value, recursive: bool) -> Result<ToolResult, String> {
    let path = resolve_path(root, args["path"].as_str().unwrap_or(""), recursive)?;
    let entry = fs::stat(&path.to_string_lossy())?;
    let mut out = format!(
        "路径：{}\n名称：{}\n类型：{}\n大小：{}（{} 字节）\n修改时间：{}（毫秒时间戳）\n",
        rel_display(root, &path),
        entry.name,
        if entry.is_dir {
            "文件夹".to_string()
        } else if entry.is_chem {
            format!("{}（{}）", entry.kind.to_uppercase(), entry.extension)
        } else if entry.extension.is_empty() {
            "文件".to_string()
        } else {
            format!("{} 文件", entry.extension)
        },
        human_size(entry.size),
        entry.size,
        entry
            .modified
            .map(|value| value.to_string())
            .unwrap_or_else(|| "未知".to_string()),
    );
    if entry.is_chem {
        let meta = crate::chem_cache::read_chem_cached(&entry.path);
        if meta.ok {
            out.push_str(&format!(
                "化学信息：化学式 {}，原子数 {}，电荷 {}，多重度 {}，基函数 {}，分子量 {:.3} g/mol\n",
                meta.formula, meta.n_atoms, meta.charge, meta.multip, meta.n_basis, meta.mol_w
            ));
            if let Some(energy) = meta.energy {
                out.push_str(&format!("能量：{energy:.10} Hartree\n"));
            }
            if let Some(job) = meta.job {
                out.push_str(&format!("作业关键词：{job}\n"));
            }
            if let Some(normal_end) = meta.normal_end {
                out.push_str(&format!(
                    "是否正常结束：{}\n",
                    if normal_end { "是" } else { "否" }
                ));
            }
        }
    }
    Ok(ToolResult {
        output: out,
        summary: if entry.is_chem {
            "含化学信息".to_string()
        } else {
            human_size(entry.size)
        },
        results: vec![tool_result_item(
            root,
            Path::new(&entry.path),
            &entry.name,
            entry.is_dir,
        )],
    })
}

// ---- 路径 ----

fn display_root(root: &Path) -> String {
    let text = root.to_string_lossy();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
}

fn rel_display(root: &Path, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) if rel.as_os_str().is_empty() => ".".to_string(),
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
        Err(_) => path.to_string_lossy().to_string(),
    }
}

/// 把模型给出的路径限制在 `root` 之内；越界或不存在的路径直接拒绝。
fn resolve_path(root: &Path, raw: &str, allow_nested: bool) -> Result<PathBuf, String> {
    let raw = raw.trim();
    let raw = if raw.is_empty() { "." } else { raw };
    let input = Path::new(raw);
    let candidate = if input.is_absolute() {
        input.to_path_buf()
    } else {
        root.join(input)
    };
    let resolved = candidate
        .canonicalize()
        .map_err(|_| format!("路径不存在：{raw}"))?;
    if !resolved.starts_with(root) {
        return Err(format!("拒绝访问当前文件夹之外的路径：{raw}"));
    }
    if !allow_nested
        && resolved != root
        && !resolved.parent().is_some_and(|parent| parent == root)
    {
        return Err(format!(
            "普通模式只能访问当前文件夹第一层的文件；请先点击「扫描」再访问子文件夹（{raw}）"
        ));
    }
    Ok(resolved)
}

fn human_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn truncate_line(text: &str, max_chars: usize) -> String {
    let mut out: String = text.chars().take(max_chars).collect();
    if text.chars().count() > max_chars {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNT: AtomicU32 = AtomicU32::new(0);

    fn tmp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let id = COUNT.fetch_add(1, Ordering::SeqCst);
        path.push(format!("wmfold_ai_{}_{}", std::process::id(), id));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn model_and_base_url_defaults() {
        assert_eq!(normalize_model(""), "deepseek-flash");
        assert_eq!(normalize_model("  deepseek-chat "), "deepseek-flash");
        assert_eq!(normalize_model("deepseek-reasoner"), "deepseek-flash");
        assert_eq!(normalize_model(" deepseek-v4-pro "), "deepseek-v4-pro");
        assert_eq!(normalize_base_url(""), "https://api.deepseek.com");
        assert_eq!(
            normalize_base_url("https://api.deepseek.com/v1/"),
            "https://api.deepseek.com"
        );
        assert_eq!(normalize_base_url("ftp://x"), "https://api.deepseek.com");
        assert_eq!(normalize_reasoning_effort("medium"), "high");
        assert_eq!(normalize_reasoning_effort("max"), "max");
        let old_reasoner = StoredAiConfig {
            model: "deepseek-reasoner".to_string(),
            ..Default::default()
        };
        assert!(stored_thinking(&old_reasoner));
    }

    #[test]
    fn endpoint_paths_are_built_once() {
        assert_eq!(
            chat_endpoint("https://api.deepseek.com"),
            "https://api.deepseek.com/chat/completions"
        );
        assert_eq!(
            chat_endpoint("https://api.deepseek.com/chat/completions"),
            "https://api.deepseek.com/chat/completions"
        );
        assert_eq!(
            models_endpoint("https://api.deepseek.com/chat/completions"),
            "https://api.deepseek.com/models"
        );
    }

    #[test]
    fn paths_are_confined_to_root() {
        let dir = tmp_dir();
        let sub = dir.join("sub");
        std::fs::create_dir(&sub).unwrap();
        let file = sub.join("a.txt");
        std::fs::write(&file, "hello").unwrap();

        let root = dir.canonicalize().unwrap();
        let resolved = resolve_path(&root, "sub/a.txt", true).unwrap();
        assert!(resolved.ends_with("a.txt"));
        assert!(resolve_path(&root, "sub/a.txt", false).is_err());
        assert!(resolve_path(&root, "..", true).is_err());
        assert!(resolve_path(&root, "sub/missing.txt", true).is_err());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn chat_loop_executes_tool_calls_and_returns_answer() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        fn read_request(stream: &mut std::net::TcpStream) -> String {
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut data: Vec<u8> = Vec::new();
            let mut buffer = [0u8; 4096];
            loop {
                let read = stream.read(&mut buffer).unwrap_or(0);
                if read == 0 {
                    break;
                }
                data.extend_from_slice(&buffer[..read]);
                if let Some(pos) = data.windows(4).position(|window| window == b"\r\n\r\n") {
                    let header_end = pos + 4;
                    let headers = String::from_utf8_lossy(&data[..header_end]).to_string();
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            if name.eq_ignore_ascii_case("content-length") {
                                value.trim().parse::<usize>().ok()
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);
                    while data.len() < header_end + length {
                        let read = stream.read(&mut buffer).unwrap_or(0);
                        if read == 0 {
                            break;
                        }
                        data.extend_from_slice(&buffer[..read]);
                    }
                    break;
                }
            }
            String::from_utf8_lossy(&data).to_string()
        }

        fn respond(stream: &mut std::net::TcpStream, body: &str) {
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.as_bytes().len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
        }

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut first, _) = listener.accept().unwrap();
            let request1 = read_request(&mut first);
            respond(
                &mut first,
                r#"{"choices":[{"message":{"role":"assistant","content":null,"reasoning_content":"先看看目录","tool_calls":[{"id":"call_1","type":"function","function":{"name":"list_dir","arguments":"{\"path\":\".\"}"}}]}}]}"#,
            );

            let (mut second, _) = listener.accept().unwrap();
            let request2 = read_request(&mut second);
            respond(
                &mut second,
                r#"{"choices":[{"message":{"role":"assistant","content":"文件夹里有一个文件。","reasoning_content":"已经知道答案"}}]}"#,
            );
            (request1, request2)
        });

        let dir = tmp_dir();
        std::fs::write(dir.join("notes.txt"), "hello").unwrap();
        let root = dir.canonicalize().unwrap();

        ensure_crypto_provider();
        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        let endpoint = format!("http://{address}/v1/chat/completions");
        let history = vec![AiChatMessage {
            role: "user".to_string(),
            content: "这个文件夹里有哪些文件？".to_string(),
            reasoning_content: None,
        }];
        let mut steps: Vec<AiToolStep> = Vec::new();
        let reply = chat_loop(
            &client,
            &endpoint,
            "test-key",
            "deepseek-flash",
            true,
            "high",
            true,
            &root,
            &history,
            &[],
            |step| steps.push(step.clone()),
        )
        .unwrap();

        assert_eq!(reply.content, "文件夹里有一个文件。");
        assert_eq!(reply.reasoning_content.as_deref(), Some("已经知道答案"));
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].status, "running");
        assert_eq!(steps[1].status, "ok");
        assert!(steps[1].summary.contains("条目"));

        let (request1, request2) = server.join().unwrap();
        assert!(request1.contains("\"tools\""));
        assert!(request1.contains("\"thinking\":{\"type\":\"enabled\"}"));
        assert!(request1.contains("\"reasoning_effort\":\"high\""));
        assert!(!request1.contains("\"temperature\""));
        assert!(request1
            .to_lowercase()
            .contains("authorization: bearer test-key"));
        assert!(request1.contains("有哪些文件"));
        assert!(request2.contains("\"tool_call_id\":\"call_1\""));
        assert!(request2.contains("\"role\":\"tool\""));
        assert!(request2.contains("\"reasoning_content\":\"先看看目录\""));
        assert!(request2.contains("notes.txt"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn chat_loop_falls_back_after_tool_round_limit() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        fn read_request(stream: &mut std::net::TcpStream) -> String {
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut data: Vec<u8> = Vec::new();
            let mut buffer = [0u8; 4096];
            loop {
                let read = stream.read(&mut buffer).unwrap_or(0);
                if read == 0 {
                    break;
                }
                data.extend_from_slice(&buffer[..read]);
                if let Some(pos) = data.windows(4).position(|window| window == b"\r\n\r\n") {
                    let header_end = pos + 4;
                    let headers = String::from_utf8_lossy(&data[..header_end]).to_string();
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            if name.eq_ignore_ascii_case("content-length") {
                                value.trim().parse::<usize>().ok()
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);
                    while data.len() < header_end + length {
                        let read = stream.read(&mut buffer).unwrap_or(0);
                        if read == 0 {
                            break;
                        }
                        data.extend_from_slice(&buffer[..read]);
                    }
                    break;
                }
            }
            String::from_utf8_lossy(&data).to_string()
        }

        fn respond(stream: &mut std::net::TcpStream, body: &str) {
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.as_bytes().len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
        }

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let mut saw_final_none = false;
            for round in 0..=MAX_TOOL_ROUNDS {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_request(&mut stream);
                if round < MAX_TOOL_ROUNDS {
                    respond(
                        &mut stream,
                        r#"{"choices":[{"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call_loop","type":"function","function":{"name":"list_dir","arguments":"{\"path\":\".\"}"}}]}}]}"#,
                    );
                } else {
                    saw_final_none = request.contains("\"tool_choice\":\"none\"");
                    respond(
                        &mut stream,
                        r#"{"choices":[{"message":{"role":"assistant","content":"已完成总结"}}]}"#,
                    );
                }
            }
            saw_final_none
        });

        let dir = tmp_dir();
        std::fs::write(dir.join("notes.txt"), "hello").unwrap();
        let root = dir.canonicalize().unwrap();
        ensure_crypto_provider();
        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        let endpoint = format!("http://{address}/v1/chat/completions");
        let history = vec![AiChatMessage {
            role: "user".to_string(),
            content: "这个文件夹里有哪些文件？".to_string(),
            reasoning_content: None,
        }];
        let reply = chat_loop(
            &client,
            &endpoint,
            "test-key",
            "deepseek-flash",
            false,
            "high",
            false,
            &root,
            &history,
            &[],
            |_| {},
        )
        .unwrap();

        assert!(server.join().unwrap(), "final request should disable tools");
        assert_eq!(reply.content, "已完成总结");
        assert!(reply.steps.len() >= MAX_TOOL_ROUNDS);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tools_list_read_search_and_info() {        let dir = tmp_dir();
        std::fs::write(dir.join("notes.txt"), "hello world\nsecond line\n").unwrap();
        let sub = dir.join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("water.xyz"), "3\nwater\nO 0.0 0.0 0.0\nH 0.7 0.6 0.0\nH -0.7 0.6 0.0\n").unwrap();

        let root = dir.canonicalize().unwrap();
        let listing = run_tool(&root, "list_dir", &json!({ "path": "." }), true).unwrap();
        assert!(listing.output.contains("notes.txt"));
        assert!(listing.output.contains("[目录] sub/"));
        assert!(listing
            .results
            .iter()
            .any(|item| item.name == "notes.txt" && !item.is_dir));
        assert!(listing
            .results
            .iter()
            .any(|item| item.name == "sub" && item.is_dir));

        let read = run_tool(&root, "read_file", &json!({ "path": "notes.txt" }), true).unwrap();
        assert!(read.output.contains("hello world"));
        assert_eq!(read.summary, "2 行 / 24 B");
        assert_eq!(read.results.len(), 1);
        assert_eq!(read.results[0].name, "notes.txt");
        assert!(!read.results[0].path.starts_with(r"\\?\"));

        let search = run_tool(&root, "search_files", &json!({ "query": "water" }), true).unwrap();
        assert!(search.output.contains("sub/water.xyz"));
        assert!(search
            .results
            .iter()
            .any(|item| item.name == "water.xyz" && !item.is_dir));

        let info = run_tool(&root, "file_info", &json!({ "path": "sub/water.xyz" }), true).unwrap();
        assert!(info.output.contains("化学式 H2O"));

        // 普通模式：子文件夹不可读、搜索不递归
        assert!(run_tool(
            &root,
            "read_file",
            &json!({ "path": "sub/water.xyz" }),
            false
        )
        .is_err());
        assert!(run_tool(&root, "list_dir", &json!({ "path": "sub" }), false).is_err());
        let shallow =
            run_tool(&root, "search_files", &json!({ "query": "water" }), false).unwrap();
        assert!(!shallow.output.contains("sub/water.xyz"));

        let outside = run_tool(&root, "read_file", &json!({ "path": "../outside.txt" }), true);
        assert!(outside.is_err());

        std::fs::remove_dir_all(&dir).ok();
    }
}
