#[test]
fn silence_does_not_become_a_successful_beat_grid() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("magic-silence-{unique}"));
    std::fs::create_dir_all(&dir).unwrap();
    let audio = dir.join("silence.wav");
    // One second of mono, 16-bit PCM at 16 kHz. No speech, percussion or beat.
    let mut wav = b"RIFF".to_vec();
    wav.extend(32036u32.to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16u32.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(16000u32.to_le_bytes());
    wav.extend(32000u32.to_le_bytes());
    wav.extend(2u16.to_le_bytes());
    wav.extend(16u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend(32000u32.to_le_bytes());
    wav.resize(32044, 0);
    std::fs::write(&audio, wav).unwrap();

    let result = super::analyze_music_native(&dir, &audio);
    let wrote_beats = dir.join("beats.json").exists();
    std::fs::remove_dir_all(dir).unwrap();
    assert!(
        result.is_err(),
        "silence was reported as measured beats: {result:?}"
    );
    assert!(!wrote_beats, "failed analysis wrote a fabricated beat file");
}
