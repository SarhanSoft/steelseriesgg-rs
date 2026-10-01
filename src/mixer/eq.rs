//! Parametric equalizer model and the shipped presets.
//!
//! Each band becomes one PipeWire filter-chain builtin biquad (`bq_peaking`, `bq_lowshelf`,
//! ...), whose controls are `Freq` (Hz), `Q` and `Gain` (dB).

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Most bands one EQ may hold (SteelSeries Sonar uses 10).
pub const MAX_BANDS: usize = 10;
/// Accepted band centre / corner frequency, Hz.
pub const FREQ_RANGE: (f32, f32) = (20.0, 20_000.0);
/// Accepted quality factor.
pub const Q_RANGE: (f32, f32) = (0.1, 10.0);
/// Accepted gain, dB (Sonar's range).
pub const GAIN_RANGE: (f32, f32) = (-12.0, 12.0);

/// Biquad filter shape of one band.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BandType {
    Peaking,
    LowShelf,
    HighShelf,
    LowPass,
    HighPass,
    Notch,
}

impl BandType {
    /// Label of the PipeWire filter-chain builtin that implements this shape.
    pub fn builtin_label(self) -> &'static str {
        match self {
            BandType::Peaking => "bq_peaking",
            BandType::LowShelf => "bq_lowshelf",
            BandType::HighShelf => "bq_highshelf",
            BandType::LowPass => "bq_lowpass",
            BandType::HighPass => "bq_highpass",
            BandType::Notch => "bq_notch",
        }
    }
}

/// One EQ band.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct EqBand {
    #[serde(rename = "type")]
    pub kind: BandType,
    /// Hz.
    pub freq: f32,
    pub q: f32,
    /// dB. Ignored by the pass and notch shapes.
    pub gain: f32,
}

impl EqBand {
    pub fn new(kind: BandType, freq: f32, q: f32, gain: f32) -> Self {
        Self { kind, freq, q, gain }
    }

    fn validate(&self, index: usize) -> Result<()> {
        let check = |name: &str, value: f32, (min, max): (f32, f32)| {
            if value.is_finite() && (min..=max).contains(&value) {
                Ok(())
            } else {
                Err(Error::InvalidConfig(format!(
                    "EQ band {} {name} {value} is outside {min}..={max}",
                    index + 1
                )))
            }
        };
        check("frequency", self.freq, FREQ_RANGE)?;
        check("Q", self.q, Q_RANGE)?;
        check("gain", self.gain, GAIN_RANGE)
    }
}

/// An equalizer: an ordered chain of up to [`MAX_BANDS`] bands.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct Eq {
    /// Name of the preset these bands came from; `None` once edited by hand.
    pub preset: Option<String>,
    pub bands: Vec<EqBand>,
}

impl Default for Eq {
    fn default() -> Self {
        Self::from_preset(EqPreset::Flat)
    }
}

impl Eq {
    pub fn from_preset(preset: EqPreset) -> Self {
        Self {
            preset: Some(preset.name().to_string()),
            bands: preset.bands(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.bands.len() > MAX_BANDS {
            return Err(Error::InvalidConfig(format!(
                "EQ has {} bands; at most {MAX_BANDS} are supported",
                self.bands.len()
            )));
        }
        self.bands.iter().enumerate().try_for_each(|(i, band)| band.validate(i))
    }

    /// Band shapes in order. Two EQs with the same shapes produce the same PipeWire graph and
    /// differ only in control values.
    pub fn shape(&self) -> Vec<BandType> {
        self.bands.iter().map(|b| b.kind).collect()
    }
}

/// Shipped EQ presets. Every preset has exactly ten bands on the same frequencies, so switching
/// presets never changes the graph shape.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EqPreset {
    Flat,
    BassBoost,
    FpsFootsteps,
    VoiceClarity,
    TrebleBoost,
    Movie,
    Music,
}

/// Band frequencies shared by all presets, Hz.
const PRESET_FREQS: [f32; MAX_BANDS] = [
    32.0, 64.0, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0,
];
/// Q of the end shelves (Butterworth) and of the octave-wide peaking bands in between.
const SHELF_Q: f32 = 0.707;
const PEAK_Q: f32 = 1.41;

impl EqPreset {
    pub const ALL: [EqPreset; 7] = [
        EqPreset::Flat,
        EqPreset::BassBoost,
        EqPreset::FpsFootsteps,
        EqPreset::VoiceClarity,
        EqPreset::TrebleBoost,
        EqPreset::Movie,
        EqPreset::Music,
    ];

    pub fn name(self) -> &'static str {
        match self {
            EqPreset::Flat => "Flat",
            EqPreset::BassBoost => "Bass Boost",
            EqPreset::FpsFootsteps => "FPS Footsteps",
            EqPreset::VoiceClarity => "Voice Clarity",
            EqPreset::TrebleBoost => "Treble Boost",
            EqPreset::Movie => "Movie",
            EqPreset::Music => "Music",
        }
    }

    /// Parse a preset name, ignoring case, spaces, `-` and `_` (`"fps-footsteps"`, `"Bass Boost"`).
    pub fn from_name(name: &str) -> Option<Self> {
        let normalize = |s: &str| {
            s.chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_ascii_lowercase()
        };
        let wanted = normalize(name);
        Self::ALL.into_iter().find(|p| normalize(p.name()) == wanted)
    }

    /// Gain per band in dB, low to high.
    fn gains(self) -> [f32; MAX_BANDS] {
        match self {
            EqPreset::Flat => [0.0; MAX_BANDS],
            EqPreset::BassBoost => [5.0, 6.0, 4.5, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            // Footsteps and reloads live around 1-5 kHz; explosions and engine rumble are cut.
            EqPreset::FpsFootsteps => [-4.0, -3.0, -2.0, -1.0, 0.0, 1.5, 4.0, 5.0, 3.0, 0.0],
            // Speech intelligibility sits at 1-4 kHz; low rumble is reduced.
            EqPreset::VoiceClarity => [-4.0, -3.0, -1.5, 0.0, 1.0, 2.5, 4.0, 3.0, 1.0, -1.0],
            EqPreset::TrebleBoost => [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 5.0, 6.0],
            EqPreset::Movie => [4.0, 3.5, 2.0, 0.0, -1.0, 0.5, 2.0, 2.0, 1.0, 1.0],
            EqPreset::Music => [3.5, 2.5, 1.0, 0.0, -1.0, -1.0, 0.0, 1.0, 2.5, 3.5],
        }
    }

    pub fn bands(self) -> Vec<EqBand> {
        let last = MAX_BANDS - 1;
        PRESET_FREQS
            .iter()
            .zip(self.gains())
            .enumerate()
            .map(|(i, (&freq, gain))| match i {
                0 => EqBand::new(BandType::LowShelf, freq, SHELF_Q, gain),
                i if i == last => EqBand::new(BandType::HighShelf, freq, SHELF_Q, gain),
                _ => EqBand::new(BandType::Peaking, freq, PEAK_Q, gain),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_has_ten_valid_bands_with_the_same_shape() {
        let flat_shape = Eq::from_preset(EqPreset::Flat).shape();
        for preset in EqPreset::ALL {
            let eq = Eq::from_preset(preset);
            assert_eq!(eq.bands.len(), 10, "{}", preset.name());
            eq.validate().unwrap();
            assert_eq!(eq.shape(), flat_shape, "{}", preset.name());
        }
        assert_eq!(flat_shape[0], BandType::LowShelf);
        assert_eq!(flat_shape[9], BandType::HighShelf);
    }

    #[test]
    fn preset_names_round_trip() {
        for preset in EqPreset::ALL {
            assert_eq!(EqPreset::from_name(preset.name()), Some(preset));
        }
        assert_eq!(EqPreset::from_name("fps-footsteps"), Some(EqPreset::FpsFootsteps));
        assert_eq!(EqPreset::from_name("BASS_BOOST"), Some(EqPreset::BassBoost));
        assert_eq!(EqPreset::from_name("loudness"), None);
    }

    #[test]
    fn validation_rejects_out_of_range_bands() {
        let mut eq = Eq::default();
        eq.bands[3].gain = 30.0;
        assert!(eq.validate().is_err());
        let mut eq = Eq::default();
        eq.bands[0].freq = f32::NAN;
        assert!(eq.validate().is_err());
        let mut eq = Eq::default();
        eq.bands.push(EqBand::new(BandType::Notch, 50.0, 4.0, 0.0));
        assert!(eq.validate().is_err(), "11 bands");
        let empty = Eq {
            preset: None,
            bands: Vec::new(),
        };
        empty.validate().unwrap();
    }

    #[test]
    fn builtin_labels_match_pipewire_names() {
        assert_eq!(BandType::Peaking.builtin_label(), "bq_peaking");
        assert_eq!(BandType::LowShelf.builtin_label(), "bq_lowshelf");
        assert_eq!(BandType::HighShelf.builtin_label(), "bq_highshelf");
    }
}
