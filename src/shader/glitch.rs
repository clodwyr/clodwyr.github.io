/// Drives a periodic glitch burst effect.
///
/// All state is pure Rust — no WASM or JS dependencies — so the state
/// machine is fully unit-testable without a browser.
///
/// Random values are injected by the caller rather than generated here so
/// that tests can use deterministic inputs.
pub struct GlitchTimer {
    /// Frames remaining until the next burst begins (0 → burst starts next tick).
    pub cooldown_remaining: u32,
    /// Frames remaining in the current burst (0 → not glitching).
    pub burst_remaining: u32,
    /// Total duration of the current burst in frames.
    pub burst_duration: u32,
    /// Intensity of the current burst: 0.2–1.0.  Set once when the burst
    /// starts and held constant for its duration.
    pub intensity: f32,
}

impl GlitchTimer {
    pub fn new() -> Self {
        GlitchTimer {
            cooldown_remaining: 360, // ~6 s at 60 fps before first glitch
            burst_remaining:    0,
            burst_duration:     1,
            intensity:          1.0,
        }
    }

    /// Advance one frame.
    ///
    /// `rand_cooldown`, `rand_burst`, and `rand_intensity` are consumed only
    /// when a burst is about to start (i.e. `cooldown_remaining` just reached
    /// 0).  The caller should source each from
    /// `(js_sys::Math::random() * 1024.0) as u32`.
    pub fn tick(&mut self, rand_cooldown: u32, rand_burst: u32, rand_intensity: u32) {
        if self.burst_remaining > 0 {
            self.burst_remaining -= 1;
        } else if self.cooldown_remaining > 0 {
            self.cooldown_remaining -= 1;
        } else {
            // Cooldown expired — start a new burst.
            self.burst_duration     = 8 + rand_burst % 12;        // 8–19 frames
            self.burst_remaining    = self.burst_duration;
            self.cooldown_remaining = 120 + rand_cooldown % 300;  // 2–7 s
            // Intensity: map rand_intensity (0–1023) onto [0.2, 1.0].
            self.intensity = 0.2 + (rand_intensity as f32 / 1023.0) * 0.8;
        }
    }

    /// Returns `true` during an active glitch burst.
    pub fn is_glitching(&self) -> bool {
        self.burst_remaining > 0
    }

    /// Linear progress through the current burst: 1.0 at start → 0.0 at end.
    /// Returns 0.0 when not glitching.
    pub fn phase(&self) -> f32 {
        if self.burst_duration == 0 || self.burst_remaining == 0 {
            return 0.0;
        }
        self.burst_remaining as f32 / self.burst_duration as f32
    }

    /// Effective glitch strength: `phase() * intensity`.
    /// This is what the shader should use to scale its effect.
    pub fn effective_phase(&self) -> f32 {
        self.phase() * self.intensity
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::GlitchTimer;

    #[test]
    fn new_timer_is_not_glitching() {
        let t = GlitchTimer::new();
        assert!(!t.is_glitching());
        assert_eq!(t.phase(), 0.0);
    }

    #[test]
    fn burst_starts_after_cooldown_expires() {
        let mut t = GlitchTimer::new();
        // Drain the initial cooldown with dummy random values.
        for _ in 0..t.cooldown_remaining {
            t.tick(0, 0, 0);
        }
        // Still not glitching yet — cooldown just reached 0 this tick.
        assert!(!t.is_glitching());
        // One more tick fires the burst.
        t.tick(0, 0, 0);
        assert!(t.is_glitching());
    }

    #[test]
    fn burst_remaining_counts_down_to_zero() {
        let mut t = GlitchTimer {
            cooldown_remaining: 0,
            burst_remaining:    0,
            burst_duration:     1,
            intensity:          1.0,
        };
        t.tick(0, 4, 0); // rand_burst=4 → burst_duration = 8+4%12 = 12
        assert!(t.is_glitching());
        let initial = t.burst_remaining;
        t.tick(0, 0, 0);
        assert_eq!(t.burst_remaining, initial - 1);
    }

    #[test]
    fn is_glitching_false_during_cooldown() {
        let t = GlitchTimer {
            cooldown_remaining: 10,
            burst_remaining:    0,
            burst_duration:     1,
            intensity:          1.0,
        };
        assert!(!t.is_glitching());
    }

    #[test]
    fn cooldown_does_not_decrement_during_burst() {
        let mut t = GlitchTimer {
            cooldown_remaining: 50,
            burst_remaining:    5,
            burst_duration:     5,
            intensity:          1.0,
        };
        t.tick(0, 0, 0);
        assert_eq!(t.cooldown_remaining, 50, "cooldown must not change during burst");
        assert_eq!(t.burst_remaining, 4);
    }

    #[test]
    fn cooldown_resets_after_burst_ends() {
        let mut t = GlitchTimer {
            cooldown_remaining: 0,
            burst_remaining:    0,
            burst_duration:     1,
            intensity:          1.0,
        };
        // Fire a burst.
        t.tick(100, 0, 0); // rand_cooldown=100 → new cooldown = 120+100%300 = 220
        let new_cooldown = t.cooldown_remaining;
        assert!(new_cooldown >= 120, "cooldown must be at least 2 s");
        // Drain the burst.
        while t.burst_remaining > 0 {
            t.tick(0, 0, 0);
        }
        assert_eq!(t.cooldown_remaining, new_cooldown, "cooldown must not change while burst drains");
    }

    #[test]
    fn phase_is_one_at_burst_start_and_falls_to_zero() {
        let mut t = GlitchTimer {
            cooldown_remaining: 0,
            burst_remaining:    0,
            burst_duration:     1,
            intensity:          1.0,
        };
        t.tick(0, 2, 0); // burst_duration = 8+2%12 = 10
        // Phase at start should be exactly 1.0.
        assert!(
            (t.phase() - 1.0).abs() < f32::EPSILON,
            "phase should be 1.0 at burst start, got {}",
            t.phase()
        );
        // Drain burst; phase should reach 0.0 afterwards.
        while t.burst_remaining > 0 {
            t.tick(0, 0, 0);
        }
        assert_eq!(t.phase(), 0.0, "phase must be 0.0 after burst ends");
    }

    #[test]
    fn phase_is_strictly_decreasing_during_burst() {
        let mut t = GlitchTimer {
            cooldown_remaining: 0,
            burst_remaining:    0,
            burst_duration:     1,
            intensity:          1.0,
        };
        t.tick(0, 7, 0); // burst_duration = 15
        let mut prev = t.phase();
        while t.burst_remaining > 0 {
            t.tick(0, 0, 0);
            let cur = t.phase();
            assert!(cur < prev, "phase should strictly decrease: {} -> {}", prev, cur);
            prev = cur;
        }
    }

    #[test]
    fn burst_duration_within_expected_range() {
        for rand_burst in 0u32..12 {
            let mut t = GlitchTimer {
                cooldown_remaining: 0,
                burst_remaining:    0,
                burst_duration:     1,
                intensity:          1.0,
            };
            t.tick(0, rand_burst, 0);
            assert!(
                t.burst_duration >= 8 && t.burst_duration <= 19,
                "burst_duration {} out of range for rand_burst={}",
                t.burst_duration, rand_burst
            );
        }
    }

    #[test]
    fn intensity_within_expected_range() {
        for rand_intensity in [0u32, 255, 511, 767, 1023] {
            let mut t = GlitchTimer {
                cooldown_remaining: 0,
                burst_remaining:    0,
                burst_duration:     1,
                intensity:          1.0,
            };
            t.tick(0, 0, rand_intensity);
            assert!(
                t.intensity >= 0.2 && t.intensity <= 1.0,
                "intensity {} out of [0.2, 1.0] for rand_intensity={}",
                t.intensity, rand_intensity
            );
        }
    }

    #[test]
    fn intensity_is_minimum_at_rand_zero() {
        let mut t = GlitchTimer {
            cooldown_remaining: 0,
            burst_remaining:    0,
            burst_duration:     1,
            intensity:          1.0,
        };
        t.tick(0, 0, 0);
        assert!(
            (t.intensity - 0.2).abs() < 0.001,
            "intensity should be 0.2 when rand_intensity=0, got {}",
            t.intensity
        );
    }

    #[test]
    fn intensity_is_maximum_at_rand_max() {
        let mut t = GlitchTimer {
            cooldown_remaining: 0,
            burst_remaining:    0,
            burst_duration:     1,
            intensity:          1.0,
        };
        t.tick(0, 0, 1023);
        assert!(
            (t.intensity - 1.0).abs() < 0.001,
            "intensity should be 1.0 when rand_intensity=1023, got {}",
            t.intensity
        );
    }

    #[test]
    fn effective_phase_scales_by_intensity() {
        let mut t = GlitchTimer {
            cooldown_remaining: 0,
            burst_remaining:    0,
            burst_duration:     1,
            intensity:          1.0,
        };
        // rand_intensity=511 → intensity ≈ 0.6
        t.tick(0, 0, 511);
        let expected = t.phase() * t.intensity;
        assert!(
            (t.effective_phase() - expected).abs() < f32::EPSILON,
            "effective_phase should equal phase * intensity"
        );
    }

    #[test]
    fn intensity_does_not_change_during_burst() {
        let mut t = GlitchTimer {
            cooldown_remaining: 0,
            burst_remaining:    0,
            burst_duration:     1,
            intensity:          1.0,
        };
        t.tick(0, 4, 511); // starts burst with a specific intensity
        let locked_intensity = t.intensity;
        while t.burst_remaining > 0 {
            t.tick(0, 0, 0); // different rand_intensity has no effect mid-burst
            assert_eq!(
                t.intensity, locked_intensity,
                "intensity must not change during a burst"
            );
        }
    }
}
