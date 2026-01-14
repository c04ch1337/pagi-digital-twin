use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

/// User-facing profile fields used for response personalization.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UserProfile {
    #[serde(default)]
    pub nickname: String,
    #[serde(default)]
    pub occupation: String,
    #[serde(default)]
    pub about: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verbosity {
    Minimal,
    Balanced,
    Detailed,
}

impl Default for Verbosity {
    fn default() -> Self {
        Self::Balanced
    }
}

/// Stored per user (`twin_id`) preferences.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserPreferences {
    #[serde(default)]
    pub profile: UserProfile,

    /// Persona preset id (e.g., "default", "nerdy").
    #[serde(default = "default_persona_preset")]
    pub persona_preset: String,

    /// Additional user custom instructions (freeform, appended to the prompt overlay).
    #[serde(default)]
    pub custom_instructions: String,

    #[serde(default)]
    pub verbosity: Verbosity,

    /// Optional toggles layered on top of base persona.
    /// Guardrails ensure these never become harassing or unsafe.
    #[serde(default)]
    pub enable_cynical: bool,
    #[serde(default)]
    pub enable_sarcastic: bool,

    /// ISO 8601 timestamp.
    #[serde(default)]
    pub updated_at: String,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            profile: UserProfile::default(),
            persona_preset: default_persona_preset(),
            custom_instructions: String::new(),
            verbosity: Verbosity::Balanced,
            enable_cynical: false,
            enable_sarcastic: false,
            updated_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

fn default_persona_preset() -> String {
    "default".to_string()
}

#[derive(Clone, Debug, Serialize)]
pub struct PersonaPreset {
    pub id: String,
    pub label: String,
    pub description: String,
    /// Prompt overlay that adjusts style/tone (not capabilities).
    pub overlay: String,
}

pub fn default_persona_presets() -> Vec<PersonaPreset> {
    vec![
        PersonaPreset {
            id: "default".to_string(),
            label: "Default".to_string(),
            description: "Neutral, concise, technical when appropriate".to_string(),
            overlay: "Style: Default\n- Be direct and helpful.\n- Prefer concise, information-dense responses.\n- Use markdown with code when helpful.".to_string(),
        },
        PersonaPreset {
            id: "professional".to_string(),
            label: "Professional".to_string(),
            description: "Clear, structured, business-appropriate".to_string(),
            overlay: "Style: Professional\n- Use clear headings and bullet points.\n- Keep tone calm and business-appropriate.\n- Avoid slang.".to_string(),
        },
        PersonaPreset {
            id: "efficient".to_string(),
            label: "Efficient".to_string(),
            description: "Minimal words, maximum utility".to_string(),
            overlay: "Style: Efficient\n- Answer in as few words as possible while being correct.\n- Prefer checklists and commands over prose.\n- Ask only the minimum necessary follow-up questions.".to_string(),
        },
        PersonaPreset {
            id: "nerdy".to_string(),
            label: "Nerdy".to_string(),
            description: "Technical, precise, deep-dive friendly (without being verbose by default)".to_string(),
            overlay: "Style: Nerdy\n- Prefer technical precision, explicit assumptions, and correct terminology.\n- When explaining: include minimal but meaningful context (why it works), then the steps.\n- Use small examples, edge-cases, and quick sanity checks when useful.\n- Keep it readable; do not ramble.".to_string(),
        },
        PersonaPreset {
            id: "candid".to_string(),
            label: "Candid".to_string(),
            description: "Direct, no-nonsense".to_string(),
            overlay: "Style: Candid\n- Be straightforward about tradeoffs and uncertainty.\n- Call out risks and missing info plainly.\n- Avoid fluff.".to_string(),
        },
        PersonaPreset {
            id: "friendly".to_string(),
            label: "Friendly".to_string(),
            description: "Warm but still technical".to_string(),
            overlay: "Style: Friendly\n- Use an approachable tone while staying accurate.\n- Prefer encouraging phrasing, but keep it technical.".to_string(),
        },
        PersonaPreset {
            id: "humorous".to_string(),
            label: "Humorous".to_string(),
            description: "Light humor (no memes), still useful".to_string(),
            overlay: "Style: Humorous\n- Light, occasional dry humor is allowed if it does not distract.\n- Never joke about sensitive topics or protected classes.".to_string(),
        },
        PersonaPreset {
            id: "philosophical".to_string(),
            label: "Philosophical".to_string(),
            description: "Explore implications, values, and tradeoffs".to_string(),
            overlay: "Style: Philosophical\n- Highlight underlying assumptions and tradeoffs.\n- Offer alternative framings when relevant.".to_string(),
        },
        PersonaPreset {
            id: "motivational".to_string(),
            label: "Motivational".to_string(),
            description: "Encouraging, goal-oriented".to_string(),
            overlay: "Style: Motivational\n- Be encouraging and action-oriented.\n- Suggest the next smallest step that makes progress.".to_string(),
        },
        PersonaPreset {
            id: "quirky".to_string(),
            label: "Quirky".to_string(),
            description: "Slightly eccentric phrasing (no cringe)".to_string(),
            overlay: "Style: Quirky\n- Small personality flourishes are okay, but keep clarity first.".to_string(),
        },
    ]
}

pub fn find_preset_by_id(id: &str) -> Option<PersonaPreset> {
    let id_norm = id.trim().to_lowercase();
    default_persona_presets()
        .into_iter()
        .find(|p| p.id == id_norm)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PreferencesFile {
    #[serde(default = "default_version")]
    version: u32,
    #[serde(default)]
    by_twin: HashMap<String, UserPreferences>,
}

fn default_version() -> u32 {
    1
}

impl Default for PreferencesFile {
    fn default() -> Self {
        Self {
            version: default_version(),
            by_twin: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PreferencesRepository {
    path: PathBuf,
}

impl PreferencesRepository {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn default_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("config")
            .join("user_preferences.json")
    }

    pub async fn load_or_init(&self) -> Result<PreferencesFile, String> {
        if tokio::fs::metadata(&self.path).await.is_ok() {
            let raw = tokio::fs::read_to_string(&self.path)
                .await
                .map_err(|e| format!("failed to read preferences file {}: {e}", self.path.display()))?;
            if !raw.trim().is_empty() {
                let parsed: PreferencesFile = serde_json::from_str(&raw).map_err(|e| {
                    format!(
                        "failed to parse preferences JSON {}: {e}",
                        self.path.display()
                    )
                })?;
                return Ok(parsed);
            }
        }

        let init = PreferencesFile::default();
        self.write(&init).await?;
        Ok(init)
    }

    pub async fn write(&self, prefs: &PreferencesFile) -> Result<(), String> {
        let dir = self
            .path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| format!("failed to create config dir {}: {e}", dir.display()))?;

        let tmp_path = dir.join(format!(
            "user_preferences.json.tmp-{}",
            uuid::Uuid::new_v4().to_string()
        ));

        let raw = serde_json::to_string_pretty(prefs)
            .map_err(|e| format!("failed to serialize preferences: {e}"))?;

        tokio::fs::write(&tmp_path, raw)
            .await
            .map_err(|e| format!(
                "failed to write temp preferences file {}: {e}",
                tmp_path.display()
            ))?;

        // Best-effort atomic replace.
        let _ = tokio::fs::remove_file(&self.path).await;
        tokio::fs::rename(&tmp_path, &self.path)
            .await
            .map_err(|e| format!(
                "failed to replace preferences file {}: {e}",
                self.path.display()
            ))?;

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct PreferencesManager {
    repo: PreferencesRepository,
    current: Arc<RwLock<PreferencesFile>>,
}

impl PreferencesManager {
    pub fn new(repo: PreferencesRepository, initial: PreferencesFile) -> Self {
        Self {
            repo,
            current: Arc::new(RwLock::new(initial)),
        }
    }

    pub async fn get_for_twin(&self, twin_id: &str) -> UserPreferences {
        let twin = twin_id.trim();
        if twin.is_empty() {
            return UserPreferences::default();
        }
        let guard = self.current.read().await;
        guard
            .by_twin
            .get(twin)
            .cloned()
            .unwrap_or_else(UserPreferences::default)
    }

    pub async fn update_for_twin(
        &self,
        twin_id: &str,
        mut updated: UserPreferences,
    ) -> Result<UserPreferences, String> {
        let twin = twin_id.trim();
        if twin.is_empty() {
            return Err("twin_id must not be empty".to_string());
        }

        // Normalize and cap free-form fields.
        updated.persona_preset = updated.persona_preset.trim().to_lowercase();
        if updated.persona_preset.is_empty() {
            updated.persona_preset = default_persona_preset();
        }
        // If preset id is unknown, fall back.
        if find_preset_by_id(&updated.persona_preset).is_none() {
            updated.persona_preset = default_persona_preset();
        }

        updated.custom_instructions = clamp_string(&updated.custom_instructions, 10_000);
        updated.profile.nickname = clamp_string(&updated.profile.nickname, 256);
        updated.profile.occupation = clamp_string(&updated.profile.occupation, 256);
        updated.profile.about = clamp_string(&updated.profile.about, 4_000);

        updated.updated_at = chrono::Utc::now().to_rfc3339();

        {
            let mut guard = self.current.write().await;
            guard.by_twin.insert(twin.to_string(), updated.clone());
            // Persist immediately so it survives restarts.
            self.repo.write(&guard).await?;
        }

        Ok(updated)
    }

    pub async fn render_prompt_overlay(&self, twin_id: &str) -> String {
        let prefs = self.get_for_twin(twin_id).await;

        let preset = find_preset_by_id(&prefs.persona_preset)
            .unwrap_or_else(|| find_preset_by_id("default").expect("default preset"));

        let mut out = String::new();
        out.push_str("[USER PERSONALIZATION]\n");
        out.push_str("These are style/tone preferences. They do NOT change your capabilities.\n\n");

        // Profile
        let has_profile = !prefs.profile.nickname.trim().is_empty()
            || !prefs.profile.occupation.trim().is_empty()
            || !prefs.profile.about.trim().is_empty();
        if has_profile {
            out.push_str("[OPERATOR PROFILE]\n");
            if !prefs.profile.nickname.trim().is_empty() {
                out.push_str(&format!("Nickname: {}\n", prefs.profile.nickname.trim()));
            }
            if !prefs.profile.occupation.trim().is_empty() {
                out.push_str(&format!("Occupation: {}\n", prefs.profile.occupation.trim()));
            }
            if !prefs.profile.about.trim().is_empty() {
                out.push_str(&format!("About: {}\n", prefs.profile.about.trim()));
            }
            out.push('\n');
        }

        // Style
        out.push_str("[STYLE & TONE]\n");
        out.push_str(&format!("PersonaPreset: {}\n", preset.label));
        out.push_str(&format!(
            "Verbosity: {}\n",
            match prefs.verbosity {
                Verbosity::Minimal => "minimal",
                Verbosity::Balanced => "balanced",
                Verbosity::Detailed => "detailed",
            }
        ));
        out.push_str(&format!("EnableCynical: {}\n", prefs.enable_cynical));
        out.push_str(&format!("EnableSarcastic: {}\n\n", prefs.enable_sarcastic));
        out.push_str(&preset.overlay);
        out.push_str("\n\n");

        if prefs.enable_cynical {
            out.push_str("CynicalMode:\n- Be mildly skeptical and risk-aware.\n- Do not be hostile; do not insult the user.\n\n");
        }
        if prefs.enable_sarcastic {
            out.push_str("SarcasmMode:\n- Sarcasm may be used lightly and sparingly.\n- Never target the user or protected classes.\n\n");
        }

        if !prefs.custom_instructions.trim().is_empty() {
            out.push_str("[CUSTOM INSTRUCTIONS]\n");
            out.push_str(prefs.custom_instructions.trim());
            out.push_str("\n\n");
        }

        // Global guardrails: keep tone safe even if toggles are enabled.
        out.push_str("[GUARDRAILS]\n");
        out.push_str("- Do not be harassing, hateful, or demeaning.\n");
        out.push_str("- If the user requests harmful/unsafe actions, refuse and offer safe alternatives.\n");
        out.push_str("- Prefer accuracy over confidence; state uncertainty when needed.\n");

        out
    }
}

fn clamp_string(s: &str, max_chars: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    trimmed.chars().take(max_chars).collect::<String>()
}

