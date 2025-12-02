# animalese-rs

Rust library for generating Animal Crossing-style "animalese" speech sounds.

Assets come courtesy of https://github.com/joshxviii/animalese-typing-desktop, also MIT Licensed.

## Features

- 8 voice types (f1-f4, m1-m4)
- Pitch shifting and randomization
- Intonation control (rising/falling pitch for questions, excitement, statements)
- Audio assets bundled with the crate
- Interactive CLI tool

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
animalese = "0.1"
```

## Usage

### Basic Example

```rust
use animalese::Animalese;

let engine = Animalese::new()?;
engine.speak("hello world")?;
```

### Custom Voice Profile

```rust
use animalese::{Animalese, VoiceProfile, VoiceType};

let mut engine = Animalese::new()?;

let profile = VoiceProfile {
    voice_type: VoiceType::M1,
    pitch_shift: -5.0,      // Lower pitch
    pitch_variation: 0.3,   // More randomness
    volume: 0.8,
    intonation: 0.0,        // No pitch glide
};

engine.set_profile(profile);
engine.speak("I'm Tom Nook")?;
```

### Intonation and Speech Patterns

```rust
use animalese::Animalese;

let engine = Animalese::new()?;

// Automatic rising intonation for questions
engine.speak_question("What's that?")?;

// Excited speech (higher pitch + rising)
engine.speak_excited("Amazing!")?;

// Statement with falling intonation
engine.speak_statement("I see.")?;

// Or manually control intonation (-1.0 to 1.0)
let mut profile = engine.profile();
profile.intonation = 0.5;  // Rising pitch over sentence
engine.set_profile(profile);
engine.speak("Going up")?;
```

### Advanced: Custom Assets

```rust
use animalese::Animalese;

// Only if you have custom audio files matching the expected format
let engine = Animalese::with_custom_assets("./my_assets/voice")?;
```

## CLI Tool

Interactive typing sounds:

```bash
cargo install animalese
animalese
```

Speak text directly:

```bash
animalese "hello world"
animalese --voice m2 --pitch=-3.0 "Tom Nook here"
animalese --intonation=0.6 "What's that?"
```

Available options:
- `--voice` (`-v`): Voice type (f1-f4, m1-m4)
- `--pitch` (`-p`): Pitch shift in semitones (-12.0 to 12.0)
- `--variation` (`-r`): Random pitch variation (0.0 to 2.0)
- `--intonation` (`-i`): Pitch glide over sentence (-1.0 falling to 1.0 rising)
- `--volume` (`-V`): Volume level (0.0 to 1.0)
- `--list` (`-l`): Show available voices
- `--test` (`-t`): Play test phrase

## License

MIT