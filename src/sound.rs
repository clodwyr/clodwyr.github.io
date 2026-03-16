use wasm_bindgen::JsValue;
use web_sys::{AudioBuffer, AudioBufferSourceNode, AudioContext, GainNode, OscillatorType};

// ── March engine ──────────────────────────────────────────────────────────────
// Cycles through 4 bass notes in time with the alien grid ticks.
// Fully pure — no AudioContext dependency — so it can be unit-tested.

/// Frequencies (Hz) for the 4-note alien march cycle.
pub const MARCH_NOTES: [f64; 4] = [130.81, 110.0, 98.0, 82.41]; // C3 A2 G2 E2

pub struct MarchEngine {
    pub note_index: usize,
    frames_since_last: u32,
}

impl MarchEngine {
    pub fn new() -> Self {
        MarchEngine {
            note_index: 0,
            frames_since_last: 0,
        }
    }

    /// Called every animation frame.
    /// Returns `Some(note_index)` when a march note should fire, `None` otherwise.
    /// `tick_interval` is frames-between-grid-moves from `ClassicSpeed`.
    pub fn tick(&mut self, tick_interval: u32) -> Option<usize> {
        self.frames_since_last += 1;
        if self.frames_since_last >= tick_interval {
            self.frames_since_last = 0;
            let note = self.note_index;
            self.note_index = (self.note_index + 1) % MARCH_NOTES.len();
            Some(note)
        } else {
            None
        }
    }
}

// ── Sound engine ──────────────────────────────────────────────────────────────

/// Duration constants (seconds)
const FIRE_DURATION:      f64 = 0.08;
const ALIEN_EXP_DURATION: f64 = 0.18;
const SHIP_EXP_DURATION:  f64 = 0.55;
const UFO_HIT_DURATION:   f64 = 0.30;
const MARCH_DURATION:     f64 = 0.06;

pub struct SoundEngine {
    pub ctx: AudioContext,
    /// Active UFO looping buffer source — kept alive while the UFO is on screen.
    ufo_source: Option<AudioBufferSourceNode>,
    ufo_gain:   Option<GainNode>,
    /// Pre-generated UFO audio buffer (generated once in new()).
    ufo_buffer: Option<AudioBuffer>,
    pub march: MarchEngine,
    pub muted: bool,
}

impl SoundEngine {
    pub fn new() -> Result<Self, JsValue> {
        let ctx = AudioContext::new()?;
        let ufo_buffer = build_ufo_buffer(&ctx).ok();
        Ok(SoundEngine { ctx, ufo_source: None, ufo_gain: None, ufo_buffer, march: MarchEngine::new(), muted: false })
    }

    /// Default mute state (false = unmuted). Used in tests without an AudioContext.
    pub fn muted_default() -> bool { false }

    /// Flip the mute flag. Returns the new state.
    pub fn toggle(muted: &mut bool) -> bool { *muted = !*muted; *muted }

    /// Must be called after the first user gesture to satisfy browser autoplay policy.
    pub fn resume(&self) {
        let _ = self.ctx.resume();
    }

    // ── Player fire ─────────────────────────────────────────────────────────

    /// Short high-pitched square-wave burst.
    pub fn play_player_fire(&self) {
        if self.muted { return; }
        self.play_osc(OscillatorType::Square, 880.0, None, 0.25, FIRE_DURATION);
    }

    // ── Alien explosion ──────────────────────────────────────────────────────

    /// Mid-pitched sawtooth burst — crisp pop.
    pub fn play_alien_explosion(&self) {
        if self.muted { return; }
        self.play_osc(OscillatorType::Sawtooth, 200.0, None, 0.4, ALIEN_EXP_DURATION);
    }

    // ── Ship explosion ───────────────────────────────────────────────────────

    /// Low sawtooth rumble, longer decay.
    pub fn play_ship_explosion(&self) {
        if self.muted { return; }
        self.play_osc(OscillatorType::Sawtooth, 80.0, None, 0.7, SHIP_EXP_DURATION);
    }

    // ── UFO flyby ────────────────────────────────────────────────────────────

    /// Start the looping sfxr-generated UFO buffer while the UFO is on screen.
    pub fn start_ufo_sound(&mut self) {
        if self.muted || self.ufo_source.is_some() { return; }
        let Some(ref buf) = self.ufo_buffer else { return };
        let Ok(source) = self.ctx.create_buffer_source() else { return };
        let Ok(gain)   = self.ctx.create_gain()           else { return };
        source.set_buffer(Some(buf));
        source.set_loop(true);
        gain.gain().set_value(1.0);
        let _ = source.connect_with_audio_node(&gain);
        let _ = gain.connect_with_audio_node(&self.ctx.destination());
        let _ = source.start();
        self.ufo_source = Some(source);
        self.ufo_gain   = Some(gain);
    }

    /// Stop the UFO looping buffer.
    pub fn stop_ufo_sound(&mut self) {
        if let Some(src) = self.ufo_source.take() {
            // Disconnect immediately rather than using the deprecated stop() API.
            let _ = src.disconnect();
        }
        self.ufo_gain = None;
    }

    // ── UFO hit ──────────────────────────────────────────────────────────────

    /// Descending sawtooth tone.
    pub fn play_ufo_hit(&self) {
        if self.muted { return; }
        self.play_osc(OscillatorType::Sawtooth, 660.0, Some(110.0), 0.3, UFO_HIT_DURATION);
    }

    // ── Alien march ──────────────────────────────────────────────────────────

    /// Play one march note (0-3 → `MARCH_NOTES` frequencies).
    pub fn play_march_note(&self, note_index: usize) {
        if self.muted { return; }
        let freq = MARCH_NOTES[note_index % MARCH_NOTES.len()] as f32;
        self.play_osc(OscillatorType::Square, freq, None, 0.2, MARCH_DURATION);
    }

    // ── Internal helper ──────────────────────────────────────────────────────

    /// Spawn a one-shot oscillator with exponential gain decay.
    /// If `end_freq` is `Some(f)`, the oscillator frequency ramps down to `f`.
    fn play_osc(
        &self,
        osc_type: OscillatorType,
        start_freq: f32,
        end_freq: Option<f32>,
        volume: f32,
        duration: f64,
    ) {
        let Ok(osc)  = self.ctx.create_oscillator() else { return };
        let Ok(gain) = self.ctx.create_gain()        else { return };
        let t = self.ctx.current_time();
        let _ = osc.set_type(osc_type);
        osc.frequency().set_value_at_time(start_freq, t).ok();
        if let Some(ef) = end_freq {
            osc.frequency().exponential_ramp_to_value_at_time(ef, t + duration).ok();
        }
        gain.gain().set_value_at_time(volume, t).ok();
        gain.gain().exponential_ramp_to_value_at_time(0.001, t + duration).ok();
        let _ = osc.connect_with_audio_node(&gain);
        let _ = gain.connect_with_audio_node(&self.ctx.destination());
        let _ = osc.start();
        let _ = osc.stop_with_when(t + duration);
    }
}

// ── sfxr buffer generation ────────────────────────────────────────────────────

/// Build the UFO looping audio buffer from the sfxr parameters supplied by the designer.
/// Implements the jsfxr synthesis algorithm (wave, ADSR envelope, freq slide, LPF, HPF).
fn build_ufo_buffer(ctx: &AudioContext) -> Result<AudioBuffer, JsValue> {
    const SAMPLE_RATE: f32 = 44100.0;

    // ── sfxr parameters ───────────────────────────────────────────────────────
    let wave_type:      u8  = 2;     // 0=square 1=sawtooth 2=sine 3=noise
    let p_base_freq:   f64 = 0.273;
    let p_freq_ramp:   f64 = 0.16125410854334565;
    let p_env_attack:  f64 = 0.418;
    let p_env_sustain: f64 = 0.24931042980550377;
    let p_env_decay:   f64 = 0.23097569329830311;
    let p_duty:        f64 = 0.21558119924656663; // only used for square wave
    let p_lpf_freq:    f64 = 0.5614087633008495;
    let p_hpf_freq:    f64 = 0.29306105792073306;
    let sound_vol:     f64 = 0.25;

    // ── envelope lengths (samples) ────────────────────────────────────────────
    let env_attack  = (p_env_attack  * p_env_attack  * 100_000.0) as usize;
    let env_sustain = (p_env_sustain * p_env_sustain * 100_000.0) as usize;
    let env_decay   = (p_env_decay   * p_env_decay   * 100_000.0) as usize;
    let total       = env_attack + env_sustain + env_decay;

    // ── frequency slide ───────────────────────────────────────────────────────
    let mut fperiod = 100.0 / (p_base_freq * p_base_freq + 0.001);
    let fslide = 1.0 - p_freq_ramp.powi(2) * 0.01;

    // ── filter state ──────────────────────────────────────────────────────────
    let fltw  = (p_lpf_freq * p_lpf_freq * p_lpf_freq * 0.1_f64).min(0.1);
    let flthp = p_hpf_freq * p_hpf_freq * 0.1;
    let mut fltp   = 0.0_f64; // LPF state
    let mut fltphp = 0.0_f64; // HPF state

    // ── oscillator ────────────────────────────────────────────────────────────
    let mut phase   = 0.0_f64;
    let mut samples = Vec::with_capacity(total);

    for i in 0..total {
        // Frequency slide
        fperiod = (fperiod * fslide).max(10.0);
        phase  += 1.0 / fperiod;
        if phase >= 1.0 { phase -= 1.0; }

        // Oscillator sample
        let osc: f64 = match wave_type {
            0 => if phase < p_duty { 0.5 } else { -0.5 }, // square
            1 => 1.0 - phase * 2.0,                        // sawtooth
            2 => (phase * std::f64::consts::TAU).sin(),    // sine
            _ => 0.0,
        };

        // LPF (1-pole)
        let prev_fltp = fltp;
        fltp += fltw * (osc - fltp);

        // HPF (leaky differentiator)
        fltphp += fltp - prev_fltp;
        fltphp -= fltphp * flthp;

        // ADSR envelope
        let env: f64 = if i < env_attack {
            i as f64 / env_attack as f64
        } else if i < env_attack + env_sustain {
            1.0
        } else {
            1.0 - (i - env_attack - env_sustain) as f64 / env_decay as f64
        };

        samples.push((fltphp * env * sound_vol * 3.0).clamp(-1.0, 1.0) as f32);
    }

    // ── write to AudioBuffer ──────────────────────────────────────────────────
    let buffer = ctx.create_buffer(1, total as u32, SAMPLE_RATE)?;
    buffer.copy_to_channel(&samples, 0)?;
    Ok(buffer)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn march_starts_at_note_0() {
        let engine = MarchEngine::new();
        assert_eq!(engine.note_index, 0);
    }

    #[test]
    fn march_fires_note_after_tick_interval_frames() {
        let mut engine = MarchEngine::new();
        // No note until tick_interval frames pass
        for _ in 0..9 {
            assert!(engine.tick(10).is_none(), "expected None before interval expires");
        }
        assert_eq!(engine.tick(10), Some(0));
    }

    #[test]
    fn march_cycles_through_4_notes_in_order() {
        let mut engine = MarchEngine::new();
        let notes: Vec<usize> = (0..4)
            .map(|_| {
                for _ in 0..9 { engine.tick(10); }
                engine.tick(10).unwrap()
            })
            .collect();
        assert_eq!(notes, vec![0, 1, 2, 3]);
    }

    #[test]
    fn march_wraps_back_to_note_0_after_4() {
        let mut engine = MarchEngine::new();
        for _ in 0..4 {
            for _ in 0..9 { engine.tick(10); }
            engine.tick(10); // consume note
        }
        // 5th note should be 0 again
        for _ in 0..9 { engine.tick(10); }
        assert_eq!(engine.tick(10), Some(0));
    }

    #[test]
    fn march_respects_different_tick_intervals() {
        let mut slow = MarchEngine::new();
        let mut fast = MarchEngine::new();
        // fast interval = 5, slow = 20
        // After 5 frames, fast fires, slow does not
        for _ in 0..4 { fast.tick(5); slow.tick(20); }
        assert_eq!(fast.tick(5), Some(0));
        assert!(slow.tick(20).is_none());
    }

    #[test]
    fn march_no_note_between_ticks() {
        let mut engine = MarchEngine::new();
        // Consume the first note
        for _ in 0..9 { engine.tick(10); }
        engine.tick(10); // note 0
        // Immediately after, should be silent until next interval
        for _ in 0..9 {
            assert!(engine.tick(10).is_none());
        }
    }

    #[test]
    fn sound_engine_starts_unmuted() {
        assert!(!SoundEngine::muted_default());
    }

    #[test]
    fn toggle_mute_enables_mute() {
        let mut muted = false;
        SoundEngine::toggle(&mut muted);
        assert!(muted);
    }

    #[test]
    fn toggle_mute_twice_restores_unmuted() {
        let mut muted = false;
        SoundEngine::toggle(&mut muted);
        SoundEngine::toggle(&mut muted);
        assert!(!muted);
    }
}
