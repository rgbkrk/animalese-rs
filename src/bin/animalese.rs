//! CLI tool for animalese text-to-speech
//!
//! Interactive mode: animalese
//! With text: animalese "hello world"
//! Piped: echo "hello" | animalese
//! With flags: animalese --voice m1 --pitch=-5.0

use animalese::{Animalese, VoiceProfile, VoiceType};
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use std::io::{self, Read};
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(name = "animalese-cli")]
#[command(about = "Real-time animalese typing sounds", long_about = None)]
struct Args {
    /// Text to speak (if not provided, enters interactive mode)
    text: Option<String>,

    /// Voice type: f1, f2, f3, f4, m1, m2, m3, m4
    #[arg(short, long, default_value = "f1")]
    voice: String,

    /// Pitch shift in semitones (-12.0 to 12.0)
    #[arg(short, long, default_value = "0.0")]
    pitch: f32,

    /// Random pitch variation (0.0 to 2.0)
    #[arg(short = 'r', long, default_value = "0.2")]
    variation: f32,

    /// Volume (0.0 to 1.0)
    #[arg(short = 'V', long, default_value = "0.65")]
    volume: f32,

    /// Path to audio assets directory (defaults to bundled assets)
    #[arg(short, long)]
    assets: Option<String>,

    /// List available voices and exit
    #[arg(short, long)]
    list: bool,

    /// Play test phrase with current settings
    #[arg(short = 't', long)]
    test: bool,
}

fn parse_voice_type(s: &str) -> Result<VoiceType, String> {
    match s.to_lowercase().as_str() {
        "f1" => Ok(VoiceType::F1),
        "f2" => Ok(VoiceType::F2),
        "f3" => Ok(VoiceType::F3),
        "f4" => Ok(VoiceType::F4),
        "m1" => Ok(VoiceType::M1),
        "m2" => Ok(VoiceType::M2),
        "m3" => Ok(VoiceType::M3),
        "m4" => Ok(VoiceType::M4),
        _ => Err(format!("Invalid voice type: {}", s)),
    }
}

fn list_voices() {
    println!("Available voices:");
    println!("  f1, f2, f3, f4  - Female voices");
    println!("  m1, m2, m3, m4  - Male voices");
}

fn interactive_mode(engine: &Animalese, args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let assets_info = args.assets.as_ref()
        .map(|s| s.as_str())
        .unwrap_or("bundled");
    println!("🎮 Animalese Interactive Mode");
    println!("   Voice: {}, Pitch: {}, Variation: {}, Assets: {}",
             args.voice, args.pitch, args.variation, assets_info);
    println!("   Type to hear sounds. Press Esc or Ctrl-C to exit.\n");

    enable_raw_mode()?;

    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let mut last_keystroke = Instant::now();

        loop {
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? {
                    match code {
                        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                            break;
                        }
                        KeyCode::Esc => {
                            break;
                        }
                        KeyCode::Char(c) => {
                            if c.is_ascii_alphabetic() {
                                // Detect typing speed
                                let now = Instant::now();
                                let time_since_last = now.duration_since(last_keystroke);
                                last_keystroke = now;

                                // If typing fast (< 100ms between keys), use shorter duration
                                let max_duration = if time_since_last < Duration::from_millis(100) {
                                    Some(Duration::from_millis(50)) // Cut off early
                                } else {
                                    None // Play full duration
                                };

                                engine.play_letter_with_duration(c, max_duration)?;
                            }
                            // Print any printable character (including spaces)
                            if !c.is_control() {
                                print!("{}", c);
                                io::Write::flush(&mut io::stdout())?;
                            }
                        }
                        KeyCode::Enter => {
                            engine.play_sfx("enter")?;
                            println!();
                        }
                        KeyCode::Backspace => {
                            engine.play_sfx("backspace")?;
                            print!("\x08 \x08"); // Move back, print space, move back again
                            io::Write::flush(&mut io::stdout())?;
                        }
                        KeyCode::Tab => {
                            engine.play_sfx("tab")?;
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    })();

    disable_raw_mode()?;
    println!("\n\n✨ Goodbye!");

    result
}

fn play_text(engine: &Animalese, text: &str) -> Result<(), Box<dyn std::error::Error>> {
    for c in text.chars() {
        if c.is_ascii_alphabetic() {
            engine.play_letter(c)?;
            std::thread::sleep(Duration::from_millis(50));
        } else if c == ' ' {
            std::thread::sleep(Duration::from_millis(100));
        } else if c == '\n' {
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    std::thread::sleep(Duration::from_millis(300));
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Handle list flag
    if args.list {
        list_voices();
        return Ok(());
    }

    // Parse voice type
    let voice_type = parse_voice_type(&args.voice)
        .map_err(|e| format!("{}\nUse --list to see available voices", e))?;

    // Create voice profile
    let profile = VoiceProfile {
        voice_type,
        pitch_shift: args.pitch,
        pitch_variation: args.variation,
        volume: args.volume,
    };

    // Initialize engine with bundled assets or custom path
    let mut engine = if let Some(custom_path) = &args.assets {
        Animalese::with_custom_assets(custom_path)
            .map_err(|e| format!("Failed to load audio files from '{}': {}", custom_path, e))?
    } else {
        Animalese::new()
            .map_err(|e| format!("Failed to load audio files: {}", e))?
    };

    engine.set_profile(profile);

    // Handle test flag
    if args.test {
        println!("🎮 Testing voice: {} (pitch: {}, variation: {}, volume: {})",
                 args.voice, args.pitch, args.variation, args.volume);
        println!("Speaking: 'hello world'");
        play_text(&engine, "hello world")?;
        return Ok(());
    }

    // Determine mode based on input
    if let Some(text) = args.text {
        // Text provided as argument
        play_text(&engine, &text)?;
    } else if atty::isnt(atty::Stream::Stdin) {
        // Piped input
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        if !buffer.trim().is_empty() {
            play_text(&engine, &buffer)?;
        }
    } else {
        // Interactive mode
        interactive_mode(&engine, &args)?;
    }

    Ok(())
}
