//! Animalese sound generator
//!
//! Recreates the Animal Crossing "animalese" speech effect by playing phonetic
//! sound sprites with pitch variation.

use rodio::{Decoder, OutputStream, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use rand::Rng;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;

/// Returns the path to bundled voice assets
///
/// Most users don't need this - just use `Animalese::new()`.
/// Only useful if you need to know where the bundled assets are located.
pub fn bundled_assets_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("audio")
        .join("voice")
}

/// Voice types available
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceType {
    F1, F2, F3, F4,
    M1, M2, M3, M4,
}

impl VoiceType {
    fn filename(&self) -> &'static str {
        match self {
            VoiceType::F1 => "f1.ogg",
            VoiceType::F2 => "f2.ogg",
            VoiceType::F3 => "f3.ogg",
            VoiceType::F4 => "f4.ogg",
            VoiceType::M1 => "m1.ogg",
            VoiceType::M2 => "m2.ogg",
            VoiceType::M3 => "m3.ogg",
            VoiceType::M4 => "m4.ogg",
        }
    }
}

/// Voice profile configuration
#[derive(Debug, Clone)]
pub struct VoiceProfile {
    pub voice_type: VoiceType,
    pub pitch_shift: f32,      // Fixed pitch shift in semitones
    pub pitch_variation: f32,  // Random variation range in semitones
    pub volume: f32,           // Volume multiplier (0.0 to 1.0)
}

impl Default for VoiceProfile {
    fn default() -> Self {
        Self {
            voice_type: VoiceType::F1,
            pitch_shift: 0.0,
            pitch_variation: 0.2,
            volume: 0.65,
        }
    }
}

/// Maps letters to their sprite positions in the audio file
/// Each letter gets 200ms starting at letter_index * 200ms
fn letter_to_sprite_time(c: char) -> Option<Duration> {
    let c = c.to_ascii_lowercase();
    if !c.is_ascii_lowercase() {
        return None;
    }

    let index = (c as u32 - 'a' as u32) as u64;
    Some(Duration::from_millis(index * 200))
}

/// Special sprite times for non-letter sounds
fn special_to_sprite_time(name: &str) -> Option<Duration> {
    match name {
        "ok" => Some(Duration::from_millis(5200)),
        "gwah" => Some(Duration::from_millis(5800)),
        "deska" => Some(Duration::from_millis(6400)),
        _ => None,
    }
}

/// SFX sprite times (600ms each)
fn sfx_to_sprite_time(name: &str) -> Option<Duration> {
    let index = match name {
        "backspace" => 0,
        "enter" => 1,
        "tab" => 2,
        "question" => 3,
        "exclamation" => 4,
        "at" => 5,
        "pound" => 6,
        "dollar" => 7,
        "caret" => 8,
        "ampersand" => 9,
        "asterisk" => 10,
        "parenthesis_open" => 11,
        "parenthesis_closed" => 12,
        "bracket_open" => 13,
        "bracket_closed" => 14,
        "brace_open" => 15,
        "brace_closed" => 16,
        "tilde" => 17,
        "default" => 18,
        "arrow_left" => 19,
        "arrow_up" => 20,
        "arrow_right" => 21,
        "arrow_down" => 22,
        "slash_forward" => 23,
        "slash_back" => 24,
        "percent" => 25,
        _ => return None,
    };
    Some(Duration::from_millis(index * 600))
}

/// Calculate playback rate from pitch shift in semitones
/// rate = 2^(semitones / 12)
fn semitones_to_rate(semitones: f32) -> f32 {
    2.0_f32.powf(semitones / 12.0)
}

/// Sound command for the playback queue
enum SoundCommand {
    Play { path: String, start: Duration, duration: Duration, apply_pitch: bool, max_duration: Option<Duration> },
    Stop,
}

/// Animalese sound engine with buffered playback
pub struct Animalese {
    _stream: OutputStream,
    voice_path: String,
    sfx_path: String,
    profile: Arc<Mutex<VoiceProfile>>,
    command_tx: Sender<SoundCommand>,
}

impl Animalese {
    /// Create a new Animalese engine with bundled assets
    ///
    /// # Example
    /// ```no_run
    /// use animalese::Animalese;
    ///
    /// let engine = Animalese::new().unwrap();
    /// engine.speak("hello world").unwrap();
    /// ```
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Self::with_custom_assets(bundled_assets_path().to_string_lossy().to_string())
    }

    /// Create an Animalese engine with custom audio assets
    ///
    /// Only use this if you have custom audio files that match the expected
    /// format (sprite sheets with 200ms letter sounds, etc).
    ///
    /// # Arguments
    /// * `assets_path` - Path to your custom assets/audio/voice directory
    ///
    /// # Example
    /// ```no_run
    /// use animalese::Animalese;
    ///
    /// let engine = Animalese::with_custom_assets("./my_assets/voice").unwrap();
    /// ```
    pub fn with_custom_assets(assets_path: impl Into<String>) -> Result<Self, Box<dyn std::error::Error>> {
        let (_stream, stream_handle) = OutputStream::try_default()?;
        let voice_path = assets_path.into();

        // SFX file is in parent directory of voice
        let sfx_path = Path::new(&voice_path)
            .parent()
            .ok_or("Invalid assets path")?
            .join("sfx.ogg")
            .to_string_lossy()
            .to_string();

        let profile = Arc::new(Mutex::new(VoiceProfile::default()));
        let profile_clone = Arc::clone(&profile);

        // Create playback queue channel
        let (command_tx, command_rx): (Sender<SoundCommand>, Receiver<SoundCommand>) = channel();

        // Spawn playback thread
        thread::spawn(move || {
            let sink = Sink::try_new(&stream_handle).expect("Failed to create sink");

            loop {
                match command_rx.recv() {
                    Ok(SoundCommand::Play { path, start, duration, apply_pitch, max_duration }) => {
                        if let Ok(file) = File::open(&path) {
                            if let Ok(source) = Decoder::new(BufReader::new(file)) {
                                // Use shorter duration if specified (for fast typing)
                                let actual_duration = max_duration.unwrap_or(duration);
                                let source = source
                                    .skip_duration(start)
                                    .take_duration(actual_duration);

                                if apply_pitch {
                                    let profile = profile_clone.lock().unwrap();
                                    let mut rng = rand::thread_rng();
                                    let random_variation = rng.gen_range(-1.0..=1.0) * profile.pitch_variation;
                                    let final_pitch = profile.pitch_shift + random_variation;
                                    let playback_rate = semitones_to_rate(final_pitch);
                                    let volume = profile.volume;
                                    drop(profile);

                                    let source = source.speed(playback_rate).amplify(volume);
                                    sink.append(source);
                                } else {
                                    let profile = profile_clone.lock().unwrap();
                                    let volume = profile.volume;
                                    drop(profile);

                                    let source = source.amplify(volume);
                                    sink.append(source);
                                }
                            }
                        }
                    }
                    Ok(SoundCommand::Stop) => {
                        sink.stop();
                    }
                    Err(_) => break, // Channel closed
                }
            }
        });

        Ok(Self {
            _stream,
            voice_path,
            sfx_path,
            profile,
            command_tx,
        })
    }

    /// Set the voice profile
    pub fn set_profile(&mut self, new_profile: VoiceProfile) {
        if let Ok(mut profile) = self.profile.lock() {
            *profile = new_profile;
        }
    }

    /// Get a copy of the current voice profile
    pub fn profile(&self) -> VoiceProfile {
        self.profile.lock().unwrap().clone()
    }

    /// Play a letter sound with the current voice profile
    pub fn play_letter(&self, c: char) -> Result<(), Box<dyn std::error::Error>> {
        self.play_letter_with_duration(c, None)
    }

    /// Play a letter sound with optional max duration (for fast typing)
    pub fn play_letter_with_duration(&self, c: char, max_duration: Option<Duration>) -> Result<(), Box<dyn std::error::Error>> {
        let sprite_time = letter_to_sprite_time(c)
            .ok_or("Not a valid letter")?;

        self.play_sprite(&self.voice_path, sprite_time, Duration::from_millis(200), true, max_duration)
    }

    /// Play a special sound (ok, gwah, deska)
    pub fn play_special(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let sprite_time = special_to_sprite_time(name)
            .ok_or("Unknown special sound")?;

        self.play_sprite(&self.voice_path, sprite_time, Duration::from_millis(600), true, None)
    }

    /// Play a sound effect (enter, backspace, etc)
    pub fn play_sfx(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let sprite_time = sfx_to_sprite_time(name)
            .ok_or("Unknown SFX sound")?;

        self.play_sprite(&self.sfx_path, sprite_time, Duration::from_millis(600), false, None)
    }

    /// Play text as animalese speech
    pub fn speak(&self, text: &str) -> Result<(), Box<dyn std::error::Error>> {
        for c in text.chars() {
            if c.is_ascii_alphabetic() {
                self.play_letter(c)?;
                // Small delay between letters to simulate speech cadence
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        Ok(())
    }

    /// Internal method to queue a sprite for playback
    fn play_sprite(&self, audio_path: &str, start: Duration, duration: Duration, apply_pitch: bool, max_duration: Option<Duration>) -> Result<(), Box<dyn std::error::Error>> {
        // Determine the full file path
        let file_path = if audio_path.ends_with(".ogg") {
            // It's already a full path to sfx.ogg
            audio_path.to_string()
        } else {
            // It's the voice directory, append the voice filename
            let profile = self.profile.lock().unwrap();
            let filename = profile.voice_type.filename();
            Path::new(audio_path).join(filename)
                .to_string_lossy()
                .to_string()
        };

        // Send play command to the queue
        self.command_tx.send(SoundCommand::Play {
            path: file_path,
            start,
            duration,
            apply_pitch,
            max_duration,
        })?;

        Ok(())
    }

    /// Stop and clear the playback queue
    pub fn stop(&self) {
        let _ = self.command_tx.send(SoundCommand::Stop);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_letter_to_sprite_time() {
        assert_eq!(letter_to_sprite_time('a'), Some(Duration::from_millis(0)));
        assert_eq!(letter_to_sprite_time('b'), Some(Duration::from_millis(200)));
        assert_eq!(letter_to_sprite_time('z'), Some(Duration::from_millis(5000)));
        assert_eq!(letter_to_sprite_time('A'), Some(Duration::from_millis(0)));
        assert_eq!(letter_to_sprite_time('1'), None);
    }

    #[test]
    fn test_semitones_to_rate() {
        assert!((semitones_to_rate(0.0) - 1.0).abs() < 0.001);
        assert!((semitones_to_rate(12.0) - 2.0).abs() < 0.001);
        assert!((semitones_to_rate(-12.0) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_special_sounds() {
        assert_eq!(special_to_sprite_time("ok"), Some(Duration::from_millis(5200)));
        assert_eq!(special_to_sprite_time("gwah"), Some(Duration::from_millis(5800)));
        assert_eq!(special_to_sprite_time("deska"), Some(Duration::from_millis(6400)));
        assert_eq!(special_to_sprite_time("unknown"), None);
    }

    #[test]
    fn test_voice_profile_default() {
        let profile = VoiceProfile::default();
        assert_eq!(profile.voice_type, VoiceType::F1);
        assert_eq!(profile.pitch_shift, 0.0);
        assert_eq!(profile.pitch_variation, 0.2);
        assert_eq!(profile.volume, 0.65);
    }
}
