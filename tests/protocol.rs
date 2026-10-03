use transcriber_api::{pcm::Decoder, protocol::{Audio, Snapshot, validate_start}};

#[test]
fn pcm_crosses_odd_boundaries() {
    let mut d = Decoder::new();
    assert_eq!(d.push(&[0, 0, 0xff]), vec![0.0]);
    assert_eq!(d.push(&[0x7f, 0x80]), vec![32767.0 / 32768.0]);
    assert!(d.finish().is_err());
}
#[test]
fn snapshots_replace_unicode_scalars() {
    let mut s = Snapshot::default();
    s.append("a🙂c");
    s.replace(2, "! ");
    assert_eq!(s.text(), "a! ");
}
#[test]
fn handshake_is_strict() {
    let a = Audio { encoding: "pcm_s16le".into(), sample_rate: 16000, channels: 1 };
    assert!(validate_start(1, &a).is_ok());
    assert!(validate_start(2, &a).is_err());
    assert!(validate_start(1, &Audio { encoding: "pcm_s16be".into(), ..a }).is_err());
}

#[test]
fn replacement_cannot_remove_committed_text() {
    let mut s = Snapshot::default();
    s.final_delta("committed");
    s.append("tail");
    s.replace(99, "new");
    assert_eq!(s.text(), "committednew");
}
