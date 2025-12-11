mod utils;
pub mod http;
pub mod realtime;
mod race_strategy;

pub use http::{QwenASRClient, DoubaoASRClient, SenseVoiceClient};
pub use realtime::{RealtimeSession, DoubaoRealtimeSession, QwenRealtimeClient, DoubaoRealtimeClient};
pub use race_strategy::{
    transcribe_with_fallback,
    transcribe_with_fallback_bytes,
    transcribe_with_fallback_clients,
};
