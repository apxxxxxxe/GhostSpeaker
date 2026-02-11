use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ===== 既存型（各crateから抽出） =====

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum Engine {
  CoeiroInkV2,
  CoeiroInkV1,
  VoiceVox,
  Lmroid,
  ShareVox,
  ItVoice,
  AivisSpeech,
  BouyomiChan,
}

impl Engine {
  pub fn port(&self) -> i32 {
    match self {
      Engine::CoeiroInkV2 => 50032,
      Engine::CoeiroInkV1 => 50031,
      Engine::VoiceVox => 50021,
      Engine::Lmroid => 49973,
      Engine::ShareVox => 50025,
      Engine::ItVoice => 49540,
      Engine::AivisSpeech => 10101,
      Engine::BouyomiChan => 50001,
    }
  }

  pub fn name(&self) -> &'static str {
    match self {
      Engine::CoeiroInkV2 => "COEIROINKv2",
      Engine::CoeiroInkV1 => "COEIROINKv1",
      Engine::VoiceVox => "VOICEVOX",
      Engine::Lmroid => "LMROID",
      Engine::ShareVox => "SHAREVOX",
      Engine::ItVoice => "ITVOICE",
      Engine::AivisSpeech => "AivisSpeech",
      Engine::BouyomiChan => "棒読みちゃん",
    }
  }
}

pub const ENGINE_LIST: &[Engine] = &[
  Engine::CoeiroInkV2,
  Engine::CoeiroInkV1,
  Engine::VoiceVox,
  Engine::Lmroid,
  Engine::ShareVox,
  Engine::ItVoice,
  Engine::AivisSpeech,
  Engine::BouyomiChan,
];

pub const NO_VOICE_UUID: &str = "dummy";

pub fn engine_from_port(port: i32) -> Option<Engine> {
  ENGINE_LIST.iter().find(|e| e.port() == port).copied()
}

fn default_one() -> f32 {
  1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceQuality {
  #[serde(default = "default_one")]
  pub speed_scale: f32,
  #[serde(default)]
  pub pitch_scale: f32,
  #[serde(default = "default_one")]
  pub intonation_scale: f32,
  #[serde(default = "default_one")]
  pub volume_scale: f32,
}

impl Default for VoiceQuality {
  fn default() -> Self {
    Self {
      speed_scale: 1.0,
      pitch_scale: 0.0,
      intonation_scale: 1.0,
      volume_scale: 1.0,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterVoice {
  pub port: i32,
  pub speaker_uuid: String,
  pub style_id: i32,
  #[serde(default)]
  pub voice_quality: VoiceQuality,
}

impl Default for CharacterVoice {
  fn default() -> Self {
    CharacterVoice::no_voice()
  }
}

impl CharacterVoice {
  pub fn no_voice() -> Self {
    Self {
      port: Engine::VoiceVox.port(),
      speaker_uuid: NO_VOICE_UUID.to_string(),
      style_id: -1,
      voice_quality: VoiceQuality::default(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerInfo {
  pub speaker_name: String,
  pub speaker_uuid: String,
  pub styles: Vec<Style>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Style {
  pub style_name: Option<String>,
  pub style_id: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostVoiceInfo {
  pub devide_by_lines: bool,
  #[serde(default)]
  pub sync_speech_to_balloon: bool,
  pub voices: Vec<Option<CharacterVoice>>,
}

impl Default for GhostVoiceInfo {
  fn default() -> Self {
    let mut v = Vec::new();
    v.resize(10, None);
    GhostVoiceInfo {
      devide_by_lines: false,
      sync_speech_to_balloon: false,
      voices: v,
    }
  }
}

impl GhostVoiceInfo {
  pub fn new(character_count: usize) -> Self {
    let mut v = Vec::new();
    v.resize(character_count, None);
    GhostVoiceInfo {
      devide_by_lines: false,
      sync_speech_to_balloon: false,
      voices: v,
    }
  }
}

// ===== IPC メッセージ型 =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
  Init {
    dll_dir: String,
    config: WorkerConfig,
  },
  Shutdown,
  SpeakAsync {
    text: String,
    ghost_name: String,
  },
  SyncStart {
    text: String,
    ghost_name: String,
  },
  SyncPoll,
  SyncCancel,
  PopDialog,
  GetEngineStatus,
  UpdateVolume {
    volume: f32,
  },
  UpdateGhostVoices {
    ghost_name: String,
    info: GhostVoiceInfo,
  },
  UpdateInitialVoice {
    voice: CharacterVoice,
  },
  UpdateSpeakByPunctuation {
    enabled: bool,
  },
  UpdateEngineAutoStart {
    engine: Engine,
    auto_start: bool,
  },
  BootEngine {
    engine: Engine,
  },
  ForceStopPlayback,
  GracefulShutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
  pub volume: f32,
  pub speak_by_punctuation: bool,
  pub ghosts_voices: HashMap<String, GhostVoiceInfo>,
  pub initial_voice: CharacterVoice,
  pub engine_auto_start: HashMap<Engine, bool>,
  pub engine_path: HashMap<Engine, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
  Ok,
  Error {
    message: String,
  },
  SyncStarted {
    first_segment: Option<SegmentInfo>,
    has_more: bool,
  },
  SyncStatus {
    state: SyncState,
  },
  Dialog {
    message: Option<String>,
  },
  EngineStatus {
    speakers_info: HashMap<Engine, Vec<SpeakerInfo>>,
    connection_status: HashMap<Engine, bool>,
    engine_paths: HashMap<Engine, String>,
  },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentInfo {
  pub text: String,
  pub raw_text: String,
  pub scope: usize,
  pub is_ellipsis: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncState {
  Playing,
  Ready {
    segment: SegmentInfo,
    has_more: bool,
  },
  Waiting,
  Complete,
}
