use anyhow::{bail, Result};

pub struct Decoder { carry: Option<u8> }
impl Decoder {
    pub fn new() -> Self { Self { carry: None } }
    pub fn push(&mut self, bytes: &[u8]) -> Vec<f32> {
        let mut b = Vec::with_capacity(bytes.len() + self.carry.is_some() as usize);
        if let Some(x) = self.carry.take() { b.push(x); }
        b.extend_from_slice(bytes);
        let n = b.len() & !1;
        if n < b.len() { self.carry = Some(b[n]); }
        b[..n].chunks_exact(2).map(|x| i16::from_le_bytes([x[0], x[1]]) as f32 / 32768.0).collect()
    }
    pub fn finish(&mut self) -> Result<()> {
        if self.carry.take().is_some() { bail!("odd trailing PCM byte") }
        Ok(())
    }
}

pub fn fixture() -> Vec<u8> {
    [0i16, 1, -1, 32767, -32768, 1600, -1600].into_iter().flat_map(i16::to_le_bytes).collect()
}
