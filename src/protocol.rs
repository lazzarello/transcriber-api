use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ClientMessage { Start { protocol: u8, audio: Audio }, Stop, Cancel, Ping }
#[derive(Debug, Deserialize)]
pub struct Audio { pub encoding: String, pub sample_rate: u32, pub channels: u8 }
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ServerMessage {
    Ready { protocol: u8, sample_rate: u32 },
    Partial { text: String, revision: u64 },
    Final { text: String, revision: u64 },
    Complete { text: String },
    Error { code: String, message: String },
    Pong,
}
pub fn validate_start(protocol: u8, a: &Audio) -> Result<(), (&'static str, &'static str)> {
    if protocol != 1 { return Err(("unsupported_protocol", "protocol 1 is required")); }
    if a.encoding != "pcm_s16le" || a.sample_rate != 16000 || a.channels != 1 {
        return Err(("unsupported_audio", "requires mono 16 kHz pcm_s16le"));
    }
    Ok(())
}

#[derive(Default, Debug)]
pub struct Snapshot { committed: String, provisional: String, pub revision: u64 }
impl Snapshot {
    pub fn append(&mut self, text: &str) { self.provisional.push_str(text); }
    pub fn final_delta(&mut self, text: &str) { self.committed.push_str(&self.provisional); self.provisional.clear(); self.committed.push_str(text); }
    pub fn replace(&mut self, backspace: usize, text: &str) {
        let keep = self.provisional.chars().count().saturating_sub(backspace);
        self.provisional = self.provisional.chars().take(keep).collect();
        self.provisional.push_str(text);
    }
    pub fn finish(&mut self) { self.committed.push_str(&self.provisional); self.provisional.clear(); }
    pub fn text(&self) -> String { format!("{}{}", self.committed, self.provisional) }
    pub fn changed(&mut self) { self.revision += 1; }
}
