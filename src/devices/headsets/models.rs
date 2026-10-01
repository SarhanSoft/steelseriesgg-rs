//! Every known SteelSeries headset: PID, name, control interface, capabilities and status
//! protocol.
//!
//! Sources (facts only; no code was copied):
//! - HeadsetControl `lib/devices/steelseries_*.hpp` (GPL-3.0): PIDs, interfaces, usage pages,
//!   command bytes, ranges, save commands, status offsets.
//! - OpenRGB `SteelSeriesArctis5Controller`, `SteelSeriesArctisNova3Controller` (GPL-2.0):
//!   lighting.
//! - nova-chatmix-linux (0BSD): Nova Pro Wireless ChatMix dial and Sonar icon.
//!
//! [EXPERIMENTAL] Nothing in this table has been tested on hardware by this project.

use super::HeadsetModel;
use super::capability::{
    BandEncoding, Capability, Choice, ChoiceDef, Encoding, Equalizer, GainPreset, Inactive, InactiveChoice,
    InactiveStyle, Level, LevelStyle, LightingPart, Preset, RawPreset, SettingId, Toggle,
};
use super::lighting::LightingStyle;
use super::report::{Command, Frame, Packet};
use super::status::{BatteryScale, StatusProtocol};
use crate::devices::product_ids::*;

/// Vendor usage page of the Nova-family control collection.
const PAGE_FFC0: Option<u16> = Some(0xffc0);

/// The ten band centres SteelSeries uses on its 10-band equalizers (HeadsetControl
/// `BAND_FREQUENCIES` for the Nova 3P and Nova 5).
const BANDS_10: &[u32] = &[32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
/// Display labels for the Nova 3's six bands. The reference gives no frequencies for this
/// model, so these are placeholders; the device only receives the gains.
const BANDS_NOVA_3: &[u32] = &[32, 125, 500, 2000, 8000, 16000];

// ---- Save sequences -------------------------------------------------------------------------

const LEGACY_SAVE: &[Packet] = &[Packet::new(Command::new(Frame::LEGACY, 0x06, 0x09), &[])];
const ARCTIS_9_SAVE: &[Packet] = &[Packet::new(Command::new(Frame::LEGACY, 0x90, 0x00), &[])];
const PRO_WIRELESS_SAVE: &[Packet] = &[Packet::new(Command::new(Frame::LEGACY, 0x90, 0xaa), &[])];
const NOVA_SAVE: &[Packet] = &[Packet::new(Command::new(Frame::NOVA, 0x00, 0x09), &[])];
const NOVA_3_SAVE: &[Packet] = &[Packet::new(Command::new(Frame::NOVA_FEATURE, 0x06, 0x09), &[])];
const NOVA_5_SAVE: &[Packet] = &[
    Packet::new(Command::new(Frame::NOVA, 0x00, 0x09), &[]),
    Packet::new(Command::new(Frame::NOVA, 0x00, 0x35), &[0x01]),
];
const NOVA_7_BLUETOOTH_SAVE: &[Packet] = &[Packet::new(Command::new(Frame::NOVA, 0x06, 0x09), &[])];
const NO_SAVE: &[Packet] = &[];

// ---- Shared option lists --------------------------------------------------------------------

const fn minutes(minutes: u8, raw: u8) -> InactiveChoice {
    InactiveChoice { minutes, raw }
}

const NOVA_PRO_INACTIVE: &[InactiveChoice] = &[
    minutes(0, 0),
    minutes(1, 1),
    minutes(5, 2),
    minutes(10, 3),
    minutes(15, 4),
    minutes(30, 5),
    minutes(60, 6),
];

const NOVA_3P_INACTIVE: &[InactiveChoice] = &[
    minutes(0, 0),
    minutes(1, 1),
    minutes(5, 5),
    minutes(10, 10),
    minutes(15, 15),
    minutes(30, 30),
    minutes(45, 45),
    minutes(60, 60),
    minutes(75, 75),
    minutes(90, 90),
];

const fn option(id: &'static str, label: &'static str, raw: u8) -> ChoiceDef {
    ChoiceDef { id, label, raw }
}

const NOVA_5_MIC_LED: &[ChoiceDef] = &[
    option("off", "Off", 0x00),
    option("low", "Low", 0x01),
    option("medium", "Medium", 0x04),
    option("high", "High", 0x0a),
];

const NOVA_7_CALL_VOLUME: &[ChoiceDef] = &[
    option("unchanged", "Leave game audio alone", 0x00),
    option("lower_12db", "Lower game audio by 12 dB", 0x01),
    option("mute_game", "Mute game audio", 0x02),
];

const NOVA_PRO_PRESETS: &[ChoiceDef] = &[
    option("preset_1", "Built-in preset 1", 0),
    option("preset_2", "Built-in preset 2", 1),
    option("preset_3", "Built-in preset 3", 2),
    option("preset_4", "Built-in preset 4", 3),
];

/// Flat, Bass, Focus, Smiley in dB - identical in the Nova 3P, Nova 5 and Nova 7 references.
const STANDARD_GAIN_PRESETS: &[GainPreset] = &[
    GainPreset {
        id: "flat",
        label: "Flat",
        gains: &[0.0; 10],
    },
    GainPreset {
        id: "bass",
        label: "Bass boost",
        gains: &[3.5, 5.5, 4.0, 1.0, -1.5, -1.5, -1.0, -1.0, -1.0, -1.0],
    },
    GainPreset {
        id: "focus",
        label: "Focus",
        gains: &[-5.0, -3.5, -1.0, -3.5, -2.5, 4.0, 6.0, -3.5, 0.0, 0.0],
    },
    GainPreset {
        id: "smiley",
        label: "Smiley",
        gains: &[3.0, 3.5, 1.5, -1.5, -4.0, -4.0, -2.5, 1.5, 3.0, 4.0],
    },
];

const ARCTIS_7_PLUS_PRESETS: &[RawPreset] = &[
    RawPreset {
        id: "flat",
        label: "Flat",
        bands: &[0x18; 10],
    },
    RawPreset {
        id: "bass",
        label: "Bass boost",
        bands: &[0x1f, 0x20, 0x1a, 0x15, 0x15, 0x16, 0x16, 0x16, 0x16, 0x23],
    },
    RawPreset {
        id: "smiley",
        label: "Smiley",
        bands: &[0x1e, 0x1b, 0x15, 0x10, 0x10, 0x13, 0x1b, 0x1e, 0x20, 0x1f],
    },
    RawPreset {
        id: "focus",
        label: "Focus",
        bands: &[0x0e, 0x16, 0x11, 0x13, 0x20, 0x24, 0x1f, 0x11, 0x18, 0x11],
    },
];

const NOVA_3_PRESETS: &[RawPreset] = &[
    RawPreset {
        id: "flat",
        label: "Flat",
        bands: &[0x14; 6],
    },
    RawPreset {
        id: "bass",
        label: "Bass boost",
        bands: &[0x1c, 0x19, 0x11, 0x14, 0x14, 0x14],
    },
    RawPreset {
        id: "smiley",
        label: "Smiley",
        bands: &[0x1a, 0x17, 0x0f, 0x12, 0x17, 0x1a],
    },
    RawPreset {
        id: "focus",
        label: "Focus",
        bands: &[0x0c, 0x0d, 0x11, 0x18, 0x1c, 0x14],
    },
];

// ---- Equalizers -----------------------------------------------------------------------------

const NOVA_PRO_EQ: Equalizer = Equalizer {
    command: Command::new(Frame::LEGACY, 0x06, 0x33),
    bands_hz: BANDS_10,
    min_db: -10.0,
    max_db: 10.0,
    step_db: 0.5,
    encoding: BandEncoding::Linear {
        baseline: 0x14,
        per_db: 2.0,
    },
    // Custom bands only apply in preset slot 4.
    prelude: &[Packet::new(Command::new(Frame::UNPADDED, 0x06, 0x2e), &[0x04])],
    save: NO_SAVE,
};

const ARCTIS_7_PLUS_EQ: Equalizer = Equalizer {
    command: Command::new(Frame::NOVA, 0x00, 0x33),
    bands_hz: BANDS_10,
    min_db: -12.0,
    max_db: 12.0,
    step_db: 0.5,
    encoding: BandEncoding::Linear {
        baseline: 0x18,
        per_db: 2.0,
    },
    prelude: &[],
    save: NOVA_SAVE,
};

const NOVA_3_EQ: Equalizer = Equalizer {
    command: Command::new(Frame::NOVA_FEATURE, 0x06, 0x33),
    bands_hz: BANDS_NOVA_3,
    min_db: -6.0,
    max_db: 6.0,
    step_db: 0.5,
    encoding: BandEncoding::Linear {
        baseline: 0x14,
        per_db: 2.0,
    },
    prelude: &[],
    save: NO_SAVE,
};

const NOVA_3P_EQ: Equalizer = Equalizer {
    command: Command::new(Frame::NOVA, 0x00, 0x33),
    bands_hz: BANDS_10,
    min_db: -12.0,
    max_db: 12.0,
    step_db: 0.1,
    encoding: BandEncoding::PeakingTenths,
    prelude: &[],
    save: NOVA_SAVE,
};

const NOVA_5_EQ: Equalizer = Equalizer {
    command: Command::new(Frame::NOVA, 0x00, 0x33),
    bands_hz: BANDS_10,
    min_db: -10.0,
    max_db: 10.0,
    step_db: 0.5,
    encoding: BandEncoding::FlaggedHalves,
    prelude: &[],
    save: NOVA_5_SAVE,
};

/// The reference declares 0.5 dB steps but writes `0x14 + gain` truncated, so only whole dB
/// reach the device; the descriptor says 1 dB to match what is sent.
const NOVA_7_EQ: Equalizer = Equalizer {
    command: Command::new(Frame::NOVA, 0x00, 0x33),
    bands_hz: BANDS_10,
    min_db: -10.0,
    max_db: 10.0,
    step_db: 1.0,
    encoding: BandEncoding::Linear {
        baseline: 0x14,
        per_db: 1.0,
    },
    prelude: &[],
    save: NO_SAVE,
};

// ---- Capability builders --------------------------------------------------------------------

const fn level(
    setting: SettingId,
    command: Command,
    max: u8,
    style: LevelStyle,
    save: &'static [Packet],
) -> Capability {
    Capability::new(
        setting,
        Encoding::Level(Level {
            command,
            max,
            style,
            save,
        }),
    )
}

const fn plain(setting: SettingId, command: Command, max: u8, save: &'static [Packet]) -> Capability {
    level(setting, command, max, LevelStyle::Plain, save)
}

const fn inactive(command: Command, style: InactiveStyle, save: &'static [Packet]) -> Capability {
    Capability::new(
        SettingId::InactiveTime,
        Encoding::Inactive(Inactive { command, style, save }),
    )
}

const fn toggle(setting: SettingId, command: Command, on: u8, off: u8, save: &'static [Packet]) -> Capability {
    Capability::new(
        setting,
        Encoding::Toggle(Toggle {
            command,
            prefix: &[],
            on,
            off,
            save,
        }),
    )
}

const fn choice(
    setting: SettingId,
    command: Command,
    options: &'static [ChoiceDef],
    save: &'static [Packet],
) -> Capability {
    Capability::new(setting, Encoding::Choice(Choice { command, options, save }))
}

const fn equalizer(eq: &'static Equalizer) -> Capability {
    Capability::new(SettingId::Equalizer, Encoding::Equalizer(eq))
}

const fn gain_presets(eq: &'static Equalizer) -> Capability {
    Capability::new(
        SettingId::EqualizerPreset,
        Encoding::Preset(Preset::Gains {
            equalizer: eq,
            presets: STANDARD_GAIN_PRESETS,
        }),
    )
}

const fn save_action(save: &'static [Packet]) -> Capability {
    Capability::new(SettingId::Save, Encoding::Save(save))
}

const fn lighting(style: LightingStyle, part: LightingPart, setting: SettingId) -> Capability {
    Capability::new(setting, Encoding::Lighting(style, part))
}

// ---- Capability lists per family ------------------------------------------------------------

const fn legacy(second: u8) -> Command {
    Command::new(Frame::LEGACY, 0x06, second)
}

const fn nova(second: u8) -> Command {
    Command::new(Frame::NOVA, 0x00, second)
}

const fn nova_feature(second: u8) -> Command {
    Command::new(Frame::NOVA_FEATURE, 0x06, second)
}

const ARCTIS_1_CAPS: &[Capability] = &[
    level(
        SettingId::Sidetone,
        legacy(0x35),
        0x12,
        LevelStyle::Switched,
        LEGACY_SAVE,
    ),
    inactive(legacy(0x53), InactiveStyle::Minutes { max: 90 }, LEGACY_SAVE),
];

const ARCTIS_7_CAPS: &[Capability] = &[
    level(
        SettingId::Sidetone,
        legacy(0x35),
        0x12,
        LevelStyle::Switched,
        LEGACY_SAVE,
    ),
    inactive(legacy(0x51), InactiveStyle::Minutes { max: 90 }, LEGACY_SAVE),
    Capability::new(
        SettingId::Lights,
        Encoding::Toggle(Toggle {
            command: legacy(0x55),
            prefix: &[0x01],
            on: 0x02,
            off: 0x00,
            save: LEGACY_SAVE,
        }),
    ),
];

const ARCTIS_9_CAPS: &[Capability] = &[
    level(
        SettingId::Sidetone,
        Command::new(Frame::LEGACY, 0x06, 0x00),
        0xfd - 0xc0,
        LevelStyle::Offset(0xc0),
        ARCTIS_9_SAVE,
    ),
    inactive(
        Command::new(Frame::LEGACY, 0x04, 0x00),
        InactiveStyle::Seconds { max_minutes: 255 },
        ARCTIS_9_SAVE,
    ),
];

const PRO_WIRELESS_CAPS: &[Capability] = &[
    plain(
        SettingId::Sidetone,
        Command::new(Frame::LEGACY, 0x39, 0xaa),
        9,
        PRO_WIRELESS_SAVE,
    ),
    inactive(
        Command::new(Frame::LEGACY, 0x3c, 0xaa),
        InactiveStyle::TenMinuteSteps { max_minutes: 250 },
        PRO_WIRELESS_SAVE,
    ),
];

const NOVA_PRO_WIRELESS_CAPS: &[Capability] = &[
    plain(SettingId::Sidetone, legacy(0x39), 3, LEGACY_SAVE),
    toggle(SettingId::Lights, legacy(0xbf), 0x0a, 0x01, LEGACY_SAVE),
    inactive(legacy(0xc1), InactiveStyle::Choices(NOVA_PRO_INACTIVE), LEGACY_SAVE),
    choice(SettingId::EqualizerPreset, legacy(0x2e), NOVA_PRO_PRESETS, LEGACY_SAVE),
    equalizer(&NOVA_PRO_EQ),
    toggle(
        SettingId::ChatMixDial,
        Command::new(Frame::NOVA_PRO_MESSAGE, 0x06, 0x49),
        0x01,
        0x00,
        NO_SAVE,
    ),
    toggle(
        SettingId::SonarIcon,
        Command::new(Frame::NOVA_PRO_MESSAGE, 0x06, 0x8d),
        0x01,
        0x00,
        NO_SAVE,
    ),
    save_action(LEGACY_SAVE),
];

const ARCTIS_7_PLUS_CAPS: &[Capability] = &[
    plain(SettingId::Sidetone, nova(0x39), 3, NOVA_SAVE),
    inactive(nova(0xa3), InactiveStyle::Minutes { max: 90 }, NOVA_SAVE),
    Capability::new(
        SettingId::EqualizerPreset,
        Encoding::Preset(Preset::RawBands {
            command: nova(0x33),
            presets: ARCTIS_7_PLUS_PRESETS,
            save: NOVA_SAVE,
        }),
    ),
    equalizer(&ARCTIS_7_PLUS_EQ),
];

const NOVA_3_CAPS: &[Capability] = &[
    plain(SettingId::Sidetone, nova_feature(0x39), 3, NOVA_3_SAVE),
    plain(SettingId::MicVolume, nova_feature(0x37), 10, NOVA_3_SAVE),
    plain(SettingId::MicMuteLedBrightness, nova_feature(0xae), 3, NOVA_3_SAVE),
    Capability::new(
        SettingId::EqualizerPreset,
        Encoding::Preset(Preset::RawBands {
            command: nova_feature(0x33),
            presets: NOVA_3_PRESETS,
            save: NOVA_3_SAVE,
        }),
    ),
    equalizer(&NOVA_3_EQ),
    lighting(LightingStyle::Nova3, LightingPart::Colors, SettingId::LedColors),
    lighting(LightingStyle::Nova3, LightingPart::Effect, SettingId::LedEffect),
    lighting(LightingStyle::Nova3, LightingPart::Speed, SettingId::LedEffectSpeed),
    save_action(NOVA_3_SAVE),
];

const NOVA_3P_CAPS: &[Capability] = &[
    plain(SettingId::Sidetone, nova(0x39), 10, NOVA_SAVE),
    inactive(nova(0xa3), InactiveStyle::Choices(NOVA_3P_INACTIVE), NOVA_SAVE),
    plain(SettingId::MicVolume, nova(0x37), 14, NOVA_SAVE),
    gain_presets(&NOVA_3P_EQ),
    equalizer(&NOVA_3P_EQ),
];

const NOVA_5_CAPS: &[Capability] = &[
    plain(SettingId::Sidetone, nova(0x39), 10, NOVA_5_SAVE),
    inactive(nova(0xa3), InactiveStyle::Minutes { max: 255 }, NO_SAVE),
    plain(SettingId::MicVolume, nova(0x37), 15, NOVA_5_SAVE),
    choice(SettingId::MicMuteLedBrightness, nova(0xae), NOVA_5_MIC_LED, NOVA_5_SAVE),
    toggle(SettingId::VolumeLimiter, nova(0x27), 0x01, 0x00, NOVA_5_SAVE),
    gain_presets(&NOVA_5_EQ),
    equalizer(&NOVA_5_EQ),
    save_action(NOVA_5_SAVE),
];

/// The Nova 7 family's capability list. `$save` is the explicit save action; `$sidetone`, when
/// given, is listed first.
macro_rules! nova7_caps {
    ($save:expr $(, $sidetone:expr)?) => {
        &[
            $($sidetone,)?
            inactive(nova(0xa3), InactiveStyle::Minutes { max: 255 }, NO_SAVE),
            gain_presets(&NOVA_7_EQ),
            equalizer(&NOVA_7_EQ),
            plain(SettingId::MicMuteLedBrightness, nova(0xae), 3, NO_SAVE),
            plain(SettingId::MicVolume, nova(0x37), 7, NO_SAVE),
            toggle(SettingId::VolumeLimiter, nova(0x3a), 0x01, 0x00, NO_SAVE),
            toggle(
                SettingId::BluetoothWhenPoweredOn,
                nova(0xb2),
                0x01,
                0x00,
                NOVA_7_BLUETOOTH_SAVE,
            ),
            choice(SettingId::BluetoothCallVolume, nova(0xb3), NOVA_7_CALL_VOLUME, NO_SAVE),
            $save,
        ]
    };
}

/// The reference sends no save after Nova 7 settings; the explicit save reuses the Gen 2's
/// `00 09` and is therefore a guess on these PIDs.
const NOVA_7_CAPS: &[Capability] = nova7_caps!(
    save_action(NOVA_SAVE).guess(),
    plain(SettingId::Sidetone, nova(0x39), 3, NO_SAVE)
);

/// The Gen 2 is the only Nova 7 the reference saves sidetone on, so its save command (`00 09`)
/// comes from the reference rather than being extrapolated.
const NOVA_7_GEN2_CAPS: &[Capability] = nova7_caps!(
    save_action(NOVA_SAVE),
    plain(SettingId::Sidetone, nova(0x39), 3, NOVA_SAVE)
);

/// The Nova 7P's dial drives sidetone in hardware and its sidetone command has no effect, so
/// the reference drops sidetone and ChatMix for it.
const NOVA_7P_CAPS: &[Capability] = nova7_caps!(save_action(NOVA_SAVE).guess());

const ARCTIS_5_CAPS: &[Capability] = &[lighting(
    LightingStyle::Arctis5,
    LightingPart::Colors,
    SettingId::LedColors,
)];

// ---- Model constructors ---------------------------------------------------------------------

const fn arctis_1(product_id: u16, name: &'static str) -> HeadsetModel {
    HeadsetModel {
        product_id,
        name,
        interface_number: 3,
        usage_page: Some(0xff43),
        capabilities: ARCTIS_1_CAPS,
        status: Some(StatusProtocol::Arctis1),
        source: "HeadsetControl steelseries_arctis_1.hpp",
    }
}

const fn arctis_7(product_id: u16, name: &'static str, wireless: bool) -> HeadsetModel {
    HeadsetModel {
        product_id,
        name,
        interface_number: 5,
        usage_page: None,
        capabilities: ARCTIS_7_CAPS,
        status: Some(StatusProtocol::Arctis7 { battery: wireless }),
        source: "HeadsetControl steelseries_arctis_7.hpp",
    }
}

const fn arctis_5(product_id: u16, name: &'static str) -> HeadsetModel {
    HeadsetModel {
        product_id,
        name,
        interface_number: 5,
        usage_page: None,
        capabilities: ARCTIS_5_CAPS,
        status: None,
        source: "OpenRGB SteelSeriesArctis5Controller",
    }
}

const fn arctis_7_plus(product_id: u16, name: &'static str) -> HeadsetModel {
    HeadsetModel {
        product_id,
        name,
        interface_number: 3,
        usage_page: PAGE_FFC0,
        capabilities: ARCTIS_7_PLUS_CAPS,
        status: Some(StatusProtocol::Arctis7Plus),
        source: "HeadsetControl steelseries_arctis_7_plus.hpp",
    }
}

const fn nova_pro_wireless(product_id: u16, name: &'static str) -> HeadsetModel {
    HeadsetModel {
        product_id,
        name,
        interface_number: 4,
        usage_page: None,
        capabilities: NOVA_PRO_WIRELESS_CAPS,
        status: Some(StatusProtocol::NovaProWireless),
        source: "HeadsetControl steelseries_arctis_nova_pro_wireless.hpp; nova-chatmix-linux",
    }
}

const fn nova_3p(product_id: u16, name: &'static str) -> HeadsetModel {
    HeadsetModel {
        product_id,
        name,
        interface_number: 3,
        usage_page: PAGE_FFC0,
        capabilities: NOVA_3P_CAPS,
        status: Some(StatusProtocol::Nova3PWireless),
        source: "HeadsetControl steelseries_arctis_nova_3p_wireless.hpp",
    }
}

const fn nova_5(product_id: u16, name: &'static str) -> HeadsetModel {
    HeadsetModel {
        product_id,
        name,
        interface_number: 3,
        usage_page: PAGE_FFC0,
        capabilities: NOVA_5_CAPS,
        status: Some(StatusProtocol::Nova5),
        source: "HeadsetControl steelseries_arctis_nova_5.hpp",
    }
}

const fn nova_7(product_id: u16, name: &'static str, battery: BatteryScale) -> HeadsetModel {
    HeadsetModel {
        product_id,
        name,
        interface_number: 3,
        usage_page: PAGE_FFC0,
        capabilities: NOVA_7_CAPS,
        status: Some(StatusProtocol::Nova7 {
            battery,
            chatmix: true,
            sidetone_read: false,
        }),
        source: "HeadsetControl steelseries_arctis_nova_7.hpp",
    }
}

const fn nova_7p(product_id: u16, name: &'static str, battery: BatteryScale) -> HeadsetModel {
    HeadsetModel {
        product_id,
        name,
        interface_number: 3,
        usage_page: PAGE_FFC0,
        capabilities: NOVA_7P_CAPS,
        status: Some(StatusProtocol::Nova7 {
            battery,
            chatmix: false,
            sidetone_read: false,
        }),
        source: "HeadsetControl steelseries_arctis_nova_7p.hpp",
    }
}

/// A PID known from the GG firmware registry with no published protocol: detected and named,
/// no settings offered.
const fn identify_only(product_id: u16, name: &'static str) -> HeadsetModel {
    HeadsetModel {
        product_id,
        name,
        interface_number: 3,
        usage_page: None,
        capabilities: &[],
        status: None,
        source: "GG firmware registry (name only; no protocol reference)",
    }
}

const STEPS_4: BatteryScale = BatteryScale::Steps(4);
const PERCENT: BatteryScale = BatteryScale::Percent;

/// Every known SteelSeries headset, ordered by PID.
pub static MODELS: &[HeadsetModel] = &[
    arctis_5(ARCTIS_5_2017, "Arctis 5 (2017)"),
    arctis_7(ARCTIS_PRO, "Arctis Pro", false),
    arctis_7(ARCTIS_7, "Arctis 7", true),
    arctis_7(ARCTIS_PRO_GAMEDAC, "Arctis Pro + GameDAC", false),
    HeadsetModel {
        product_id: ARCTIS_PRO_WIRELESS,
        name: "Arctis Pro Wireless",
        interface_number: 0,
        usage_page: None,
        capabilities: PRO_WIRELESS_CAPS,
        status: Some(StatusProtocol::ProWireless),
        source: "HeadsetControl steelseries_arctis_pro_wireless.hpp",
    },
    arctis_5(ARCTIS_5, "Arctis 5"),
    arctis_7(ARCTIS_7_2019, "Arctis 7 (2019)", true),
    arctis_1(ARCTIS_1_WIRELESS, "Arctis 1 Wireless"),
    arctis_1(ARCTIS_1_WIRELESS_XBOX, "Arctis 1 Wireless (Xbox)"),
    HeadsetModel {
        product_id: ARCTIS_9,
        name: "Arctis 9",
        interface_number: 0,
        usage_page: None,
        capabilities: ARCTIS_9_CAPS,
        status: Some(StatusProtocol::Arctis9),
        source: "HeadsetControl steelseries_arctis_9.hpp",
    },
    identify_only(ARCTIS_NOVA_PRO_WIRED, "Arctis Nova Pro"),
    arctis_1(ARCTIS_7P, "Arctis 7P"),
    arctis_1(ARCTIS_7X, "Arctis 7X"),
    nova_pro_wireless(ARCTIS_NOVA_PRO_WIRELESS, "Arctis Nova Pro Wireless"),
    nova_pro_wireless(ARCTIS_NOVA_PRO_WIRELESS_XBOX, "Arctis Nova Pro Wireless (Xbox)"),
    HeadsetModel {
        product_id: ARCTIS_NOVA_3,
        name: "Arctis Nova 3",
        interface_number: 4,
        usage_page: PAGE_FFC0,
        capabilities: NOVA_3_CAPS,
        status: None,
        source: "HeadsetControl steelseries_arctis_nova_3.hpp; OpenRGB SteelSeriesArctisNova3Controller",
    },
    nova_7(ARCTIS_NOVA_7, "Arctis Nova 7", STEPS_4),
    nova_7(ARCTIS_NOVA_7X, "Arctis Nova 7X", STEPS_4),
    nova_7p(ARCTIS_NOVA_7P, "Arctis Nova 7P", STEPS_4),
    arctis_7_plus(ARCTIS_7_PLUS, "Arctis 7+"),
    arctis_7_plus(ARCTIS_7_PLUS_PS5, "Arctis 7P+"),
    arctis_7_plus(ARCTIS_7_PLUS_XBOX, "Arctis 7X+"),
    nova_5(ARCTIS_NOVA_5, "Arctis Nova 5"),
    arctis_7_plus(ARCTIS_7_PLUS_DESTINY, "Arctis 7+ Destiny 2 Edition"),
    nova_7(ARCTIS_NOVA_7_DIABLO_IV, "Arctis Nova 7 Diablo IV Edition", STEPS_4),
    nova_5(ARCTIS_NOVA_5X, "Arctis Nova 5X"),
    nova_7(ARCTIS_NOVA_7X_V2, "Arctis Nova 7X v2", PERCENT),
    nova_3p(ARCTIS_NOVA_3P_WIRELESS, "Arctis Nova 3P Wireless"),
    nova_3p(ARCTIS_NOVA_3X_WIRELESS, "Arctis Nova 3X Wireless"),
    nova_7(ARCTIS_NOVA_7_WOW, "Arctis Nova 7 WoW Edition", STEPS_4),
    HeadsetModel {
        product_id: ARCTIS_NOVA_7_GEN2,
        name: "Arctis Nova 7 Wireless Gen 2",
        interface_number: 3,
        usage_page: PAGE_FFC0,
        capabilities: NOVA_7_GEN2_CAPS,
        status: Some(StatusProtocol::Nova7 {
            battery: PERCENT,
            chatmix: true,
            sidetone_read: true,
        }),
        source: "HeadsetControl steelseries_arctis_nova_7.hpp",
    },
    identify_only(ARCTIS_NOVA_PRO_OMNI, "Arctis Nova Pro Omni"),
    nova_7p(ARCTIS_NOVA_7P_GEN2, "Arctis Nova 7P Gen 2", PERCENT),
    nova_7(ARCTIS_NOVA_7X_GEN2, "Arctis Nova 7X Gen 2", PERCENT),
    nova_7(ARCTIS_NOVA_7_V2, "Arctis Nova 7", PERCENT),
    nova_7(ARCTIS_NOVA_7X_ALT, "Arctis Nova 7X", STEPS_4),
    nova_7(ARCTIS_NOVA_7X_ALT_V2, "Arctis Nova 7X", PERCENT),
    nova_7p(ARCTIS_NOVA_7P_V2, "Arctis Nova 7P", PERCENT),
    nova_7(ARCTIS_NOVA_7_DIABLO_IV_V2, "Arctis Nova 7 Diablo IV Edition", PERCENT),
    nova_7(ARCTIS_NOVA_7X_V2_ALT, "Arctis Nova 7X v2", PERCENT),
    HeadsetModel {
        product_id: ARCTIS_GAMEBUDS,
        name: "Arctis GameBuds",
        interface_number: 3,
        usage_page: PAGE_FFC0,
        capabilities: &[],
        status: Some(StatusProtocol::GameBuds),
        source: "HeadsetControl steelseries_arctis_gamebuds.hpp",
    },
];
