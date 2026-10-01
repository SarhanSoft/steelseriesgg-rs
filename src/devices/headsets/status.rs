//! Live status: which requests each headset family answers, and how to read the answers.
//!
//! Every parser is a pure function of the response bytes, so each one is pinned by a unit test
//! with a sample response. Byte offsets, scales and status values come from HeadsetControl
//! (`lib/devices/steelseries_*.hpp`, GPL-3.0, facts only) and, for the Nova Pro Wireless ChatMix
//! dial, from nova-chatmix-linux (0BSD).
//!
//! [EXPERIMENTAL] Not tested on hardware by this project.

use super::report::{Command, Frame, Report};
use crate::devices::settings::{ChatMix, DeviceStatus};

/// How a model reports its battery level.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BatteryScale {
    /// Discrete steps `0..=max`, scaled to a percentage.
    Steps(u8),
    /// Already a percentage.
    Percent,
}

/// The status protocol a model speaks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusProtocol {
    /// Arctis 1 Wireless / 7X / 7P: `06 12`, battery at 3, link at 2.
    Arctis1,
    /// Arctis 7 / Pro: `06 18` for battery (wireless models only), `06 24` for ChatMix.
    Arctis7 { battery: bool },
    /// Arctis 9: `00 20`, battery, charging and ChatMix in one answer.
    Arctis9,
    /// Arctis Pro Wireless: `41 AA` for the link, then `40 AA` for the battery.
    ProWireless,
    /// Arctis Nova Pro Wireless: `06 B0`; ChatMix arrives as unsolicited base-station messages.
    NovaProWireless,
    /// Arctis 7+: `00 B0`, battery in steps of 0-4.
    Arctis7Plus,
    /// Arctis Nova 3P / 3X Wireless: `00 B0`, battery in percent.
    Nova3PWireless,
    /// Arctis Nova 5 / 5X: `00 B0`, battery in percent, ChatMix at 5-6.
    Nova5,
    /// Arctis Nova 7 family: `00 B0`; Gen 2 also answers `00 20` with its sidetone level.
    Nova7 {
        battery: BatteryScale,
        chatmix: bool,
        sidetone_read: bool,
    },
    /// Arctis GameBuds: `00 B0`, one status and battery byte per bud.
    GameBuds,
}

/// Which incoming reports answer a query. Others are skipped as unsolicited.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Accept {
    /// The first report that arrives.
    First,
    /// A report with `value` at `index`; if none arrives in time, the first report.
    Prefer { index: usize, value: u8 },
    /// Only a report with `value` at `index`.
    Only { index: usize, value: u8 },
}

impl Accept {
    pub fn matches(self, report: &[u8]) -> bool {
        match self {
            Self::First => true,
            Self::Prefer { index, value } | Self::Only { index, value } => report.get(index) == Some(&value),
        }
    }

    pub fn allows_fallback(self) -> bool {
        matches!(self, Self::First | Self::Prefer { .. })
    }
}

/// How to read one answer into a [`DeviceStatus`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Parser {
    Arctis1,
    Arctis7Battery,
    Arctis7ChatMix,
    Arctis9,
    ProWirelessLink,
    ProWirelessBattery,
    NovaProWireless,
    Arctis7Plus,
    Nova3PWireless,
    Nova5,
    Nova7 { battery: BatteryScale, chatmix: bool },
    Nova7Sidetone,
    GameBuds,
}

/// Whether later queries should still run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Parsed {
    Continue,
    /// The headset is off or unlinked; later queries would only time out.
    Stop,
}

/// One request and how to read its answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Query {
    pub request: Report,
    pub accept: Accept,
    pub parser: Parser,
}

const ARCTIS1_STATUS: Command = Command::new(Frame::UNPADDED, 0x06, 0x12);
const ARCTIS7_BATTERY: Command = Command::new(Frame::UNPADDED, 0x06, 0x18);
const ARCTIS7_CHATMIX: Command = Command::new(Frame::UNPADDED, 0x06, 0x24);
const ARCTIS9_STATUS: Command = Command::new(Frame::UNPADDED, 0x00, 0x20);
const PRO_WIRELESS_LINK: Command = Command::new(Frame::LEGACY, 0x41, 0xaa);
const PRO_WIRELESS_BATTERY: Command = Command::new(Frame::LEGACY, 0x40, 0xaa);
const NOVA_PRO_STATUS: Command = Command::new(Frame::LEGACY, 0x06, 0xb0);
const NOVA_STATUS: Command = Command::new(Frame::UNPADDED, 0x00, 0xb0);
const NOVA7_SETTINGS: Command = Command::new(Frame::NOVA, 0x00, 0x20);

/// First byte of a Nova-family status answer (it echoes the request).
const NOVA_STATUS_TAG: u8 = 0xb0;
/// First byte of the Nova 7 Gen 2 audio-settings answer.
const NOVA7_SETTINGS_TAG: u8 = 0x20;

/// Nova Pro Wireless base station to host direction byte (nova-chatmix-linux `RX`).
const NOVA_PRO_RX: u8 = 0x07;
/// Nova Pro Wireless ChatMix message (nova-chatmix-linux `OPT_CHATMIX`).
const NOVA_PRO_CHATMIX: u8 = 0x45;
/// Messages the Nova Pro Wireless base station sends on its own: ChatMix, volume, EQ band and
/// EQ preset (nova-chatmix-linux `OPT_CHATMIX`, `OPT_VOLUME`, `OPT_EQ`, `OPT_EQ_PRESET`).
const NOVA_PRO_EVENTS: [u8; 4] = [NOVA_PRO_CHATMIX, 0x25, 0x31, 0x2e];

impl StatusProtocol {
    /// The requests to send, in order.
    pub fn queries(self) -> Vec<Query> {
        let nova = |parser| Query {
            request: NOVA_STATUS.report(&[]),
            accept: Accept::Prefer {
                index: 0,
                value: NOVA_STATUS_TAG,
            },
            parser,
        };
        let first = |command: Command, parser| Query {
            request: command.report(&[]),
            accept: Accept::First,
            parser,
        };
        match self {
            Self::Arctis1 => vec![first(ARCTIS1_STATUS, Parser::Arctis1)],
            Self::Arctis7 { battery } => {
                let mut queries = Vec::with_capacity(2);
                if battery {
                    queries.push(first(ARCTIS7_BATTERY, Parser::Arctis7Battery));
                }
                queries.push(first(ARCTIS7_CHATMIX, Parser::Arctis7ChatMix));
                queries
            }
            Self::Arctis9 => vec![first(ARCTIS9_STATUS, Parser::Arctis9)],
            Self::ProWireless => vec![
                first(PRO_WIRELESS_LINK, Parser::ProWirelessLink),
                first(PRO_WIRELESS_BATTERY, Parser::ProWirelessBattery),
            ],
            Self::NovaProWireless => vec![first(NOVA_PRO_STATUS, Parser::NovaProWireless)],
            Self::Arctis7Plus => vec![nova(Parser::Arctis7Plus)],
            Self::Nova3PWireless => vec![nova(Parser::Nova3PWireless)],
            Self::Nova5 => vec![nova(Parser::Nova5)],
            Self::Nova7 {
                battery,
                chatmix,
                sidetone_read,
            } => {
                let mut queries = vec![nova(Parser::Nova7 { battery, chatmix })];
                if sidetone_read {
                    queries.push(Query {
                        request: NOVA7_SETTINGS.report(&[]),
                        accept: Accept::Only {
                            index: 0,
                            value: NOVA7_SETTINGS_TAG,
                        },
                        parser: Parser::Nova7Sidetone,
                    });
                }
                queries
            }
            Self::GameBuds => vec![nova(Parser::GameBuds)],
        }
    }

    /// Whether this protocol pushes unsolicited reports that [`parse_event`] understands.
    pub fn has_events(self) -> bool {
        matches!(self, Self::NovaProWireless)
    }

    /// Whether `report` is an unsolicited message of this protocol, to be skipped while waiting
    /// for an answer rather than taken as the answer.
    pub fn is_event(self, report: &[u8]) -> bool {
        self.has_events()
            && report.first() == Some(&NOVA_PRO_RX)
            && report.get(1).is_some_and(|kind| NOVA_PRO_EVENTS.contains(kind))
    }
}

/// Read an unsolicited ChatMix message (Nova Pro Wireless: `07 45 game chat`).
pub fn parse_event(protocol: StatusProtocol, report: &[u8]) -> Option<ChatMix> {
    if !protocol.has_events() {
        return None;
    }
    match report {
        [NOVA_PRO_RX, NOVA_PRO_CHATMIX, game, chat, ..] => Some(ChatMix {
            game: (*game).min(100),
            chat: (*chat).min(100),
        }),
        _ => None,
    }
}

/// HeadsetControl's `map()` onto 0-100: clamp to the input range, then scale with integer
/// division.
pub fn scale_to_percent(raw: u8, in_min: u8, in_max: u8) -> u8 {
    if in_max <= in_min {
        return 0;
    }
    let clamped = raw.clamp(in_min, in_max);
    let scaled = u32::from(clamped - in_min) * 100 / u32::from(in_max - in_min);
    u8::try_from(scaled).unwrap_or(100)
}

fn battery(raw: u8, scale: BatteryScale) -> u8 {
    match scale {
        BatteryScale::Steps(max) => scale_to_percent(raw, 0, max),
        BatteryScale::Percent => raw.min(100),
    }
}

/// Arctis 7 ChatMix bytes run 191-255 with 0 meaning "this side at maximum".
fn arctis7_chatmix_percent(raw: u8) -> u8 {
    if raw == 0 { 100 } else { scale_to_percent(raw, 191, 255) }
}

fn byte(response: &[u8], index: usize) -> Option<u8> {
    response.get(index).copied()
}

impl Parser {
    /// Fill `status` from `response`. Fields the response does not carry are left untouched.
    pub fn parse(self, response: &[u8], status: &mut DeviceStatus) -> Parsed {
        match self {
            Self::Arctis1 => {
                let (Some(link), Some(level)) = (byte(response, 2), byte(response, 3)) else {
                    return Parsed::Continue;
                };
                const OFFLINE: u8 = 0x01;
                if link == OFFLINE {
                    status.wireless_connected = Some(false);
                    return Parsed::Stop;
                }
                status.wireless_connected = Some(true);
                status.battery_percent = Some(level.min(100));
            }
            Self::Arctis7Battery => {
                if let Some(level) = byte(response, 2) {
                    status.battery_percent = Some(level.min(100));
                }
            }
            Self::Arctis7ChatMix => {
                if let (Some(game), Some(chat)) = (byte(response, 2), byte(response, 3)) {
                    status.chatmix = Some(ChatMix {
                        game: arctis7_chatmix_percent(game),
                        chat: arctis7_chatmix_percent(chat),
                    });
                }
            }
            Self::Arctis9 => {
                const BATTERY_MIN: u8 = 0x64;
                const BATTERY_MAX: u8 = 0x9a;
                const OFFLINE: u8 = 0xff;
                const CHARGING: u8 = 0x01;
                const CHATMIX_MAX: u8 = 19;
                if let (Some(game), Some(chat)) = (byte(response, 9), byte(response, 10)) {
                    status.chatmix = Some(ChatMix {
                        game: scale_to_percent(game, 0, CHATMIX_MAX),
                        chat: scale_to_percent(chat, 0, CHATMIX_MAX),
                    });
                }
                let (Some(level), Some(state)) = (byte(response, 3), byte(response, 4)) else {
                    return Parsed::Continue;
                };
                if state == OFFLINE {
                    status.wireless_connected = Some(false);
                    return Parsed::Continue;
                }
                status.wireless_connected = Some(true);
                status.charging = Some(state == CHARGING);
                status.battery_percent = Some(scale_to_percent(level, BATTERY_MIN, BATTERY_MAX));
            }
            Self::ProWirelessLink => {
                const OFFLINE: u8 = 0x02;
                let Some(link) = byte(response, 0) else {
                    return Parsed::Continue;
                };
                status.wireless_connected = Some(link != OFFLINE);
                if link == OFFLINE {
                    return Parsed::Stop;
                }
            }
            Self::ProWirelessBattery => {
                if let Some(level) = byte(response, 0) {
                    status.battery_percent = Some(scale_to_percent(level, 0, 4));
                }
            }
            Self::NovaProWireless => {
                const OFFLINE: u8 = 0x01;
                const CABLE_CHARGING: u8 = 0x02;
                let (Some(level), Some(state)) = (byte(response, 6), byte(response, 15)) else {
                    return Parsed::Continue;
                };
                if state == OFFLINE {
                    status.wireless_connected = Some(false);
                    return Parsed::Stop;
                }
                status.wireless_connected = Some(true);
                status.charging = Some(state == CABLE_CHARGING);
                status.battery_percent = Some(scale_to_percent(level, 0, 8));
            }
            Self::Arctis7Plus => {
                const OFFLINE: u8 = 0x01;
                const CHARGING: u8 = 0x01;
                read_nova_chatmix(response, 4, status, |raw| scale_to_percent(raw, 0, 100));
                let (Some(link), Some(level), Some(state)) = (byte(response, 1), byte(response, 2), byte(response, 3))
                else {
                    return Parsed::Continue;
                };
                if link == OFFLINE {
                    status.wireless_connected = Some(false);
                    return Parsed::Continue;
                }
                status.wireless_connected = Some(true);
                status.charging = Some(state == CHARGING);
                status.battery_percent = Some(scale_to_percent(level, 0, 4));
            }
            Self::Nova3PWireless | Self::Nova5 => {
                const OFFLINE: u8 = 0x02;
                const CHARGING: u8 = 0x01;
                if self == Self::Nova5 {
                    read_nova_chatmix(response, 5, status, |raw| raw.min(100));
                }
                let (Some(link), Some(level), Some(state)) = (byte(response, 1), byte(response, 3), byte(response, 4))
                else {
                    return Parsed::Continue;
                };
                if link == OFFLINE {
                    status.wireless_connected = Some(false);
                    return Parsed::Continue;
                }
                status.wireless_connected = Some(true);
                status.charging = Some(state == CHARGING);
                status.battery_percent = Some(level.min(100));
            }
            Self::Nova7 {
                battery: scale,
                chatmix,
            } => {
                const OFFLINE: u8 = 0x00;
                const CHARGING: [u8; 2] = [0x01, 0x02];
                if chatmix {
                    read_nova_chatmix(response, 4, status, |raw| raw.min(100));
                }
                let (Some(level), Some(state)) = (byte(response, 2), byte(response, 3)) else {
                    return Parsed::Continue;
                };
                if state == OFFLINE {
                    status.wireless_connected = Some(false);
                    return Parsed::Continue;
                }
                status.wireless_connected = Some(true);
                status.charging = Some(CHARGING.contains(&state));
                status.battery_percent = Some(battery(level, scale));
            }
            Self::Nova7Sidetone => {
                const LEVELS: u8 = 4;
                if let Some(level) = byte(response, 2).filter(|l| *l < LEVELS) {
                    status.extra.insert("sidetone".to_owned(), level.to_string());
                }
            }
            Self::GameBuds => {
                const ACTIVE: u8 = 0x03;
                let buds = [("left", 3, 5), ("right", 4, 6)];
                let mut lowest: Option<u8> = None;
                let mut seen_any = false;
                for (side, state_at, level_at) in buds {
                    let (Some(state), Some(level)) = (byte(response, state_at), byte(response, level_at)) else {
                        continue;
                    };
                    seen_any = true;
                    if state == ACTIVE {
                        let level = level.min(100);
                        status.extra.insert(format!("battery_{side}"), level.to_string());
                        lowest = Some(lowest.map_or(level, |l| l.min(level)));
                    }
                }
                if seen_any {
                    status.wireless_connected = Some(lowest.is_some());
                    status.battery_percent = lowest;
                }
            }
        }
        Parsed::Continue
    }
}

/// Nova-family ChatMix: game volume at `at`, chat volume at `at + 1`. Read even when the headset
/// is off, as the reference notes the dongle always reports it.
fn read_nova_chatmix(response: &[u8], at: usize, status: &mut DeviceStatus, to_percent: impl Fn(u8) -> u8) {
    if let (Some(game), Some(chat)) = (byte(response, at), byte(response, at + 1)) {
        status.chatmix = Some(ChatMix {
            game: to_percent(game),
            chat: to_percent(chat),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(parser: Parser, response: &[u8]) -> (DeviceStatus, Parsed) {
        let mut status = DeviceStatus::default();
        let outcome = parser.parse(response, &mut status);
        (status, outcome)
    }

    fn padded(prefix: &[u8]) -> Vec<u8> {
        let mut v = prefix.to_vec();
        v.resize(64, 0);
        v
    }

    #[test]
    fn scale_matches_headsetcontrol_map() {
        assert_eq!(scale_to_percent(2, 0, 4), 50);
        assert_eq!(scale_to_percent(3, 0, 8), 37);
        assert_eq!(scale_to_percent(0x9a, 0x64, 0x9a), 100);
        assert_eq!(scale_to_percent(0x50, 0x64, 0x9a), 0);
        assert_eq!(scale_to_percent(0xff, 0x64, 0x9a), 100);
        assert_eq!(scale_to_percent(5, 3, 3), 0);
    }

    #[test]
    fn arctis1_reads_battery_and_link() {
        let (s, outcome) = parse(Parser::Arctis1, &[0x06, 0x12, 0x00, 0x42, 0, 0, 0, 0]);
        assert_eq!(outcome, Parsed::Continue);
        assert_eq!(s.battery_percent, Some(0x42));
        assert_eq!(s.wireless_connected, Some(true));
        assert_eq!(s.charging, None);

        let (s, outcome) = parse(Parser::Arctis1, &[0x06, 0x12, 0x01, 0x42]);
        assert_eq!(outcome, Parsed::Stop);
        assert_eq!(s.battery_percent, None);
        assert_eq!(s.wireless_connected, Some(false));
    }

    #[test]
    fn arctis7_reads_battery_and_chatmix() {
        let (s, _) = parse(Parser::Arctis7Battery, &[0x06, 0x18, 77, 0, 0, 0, 0, 0]);
        assert_eq!(s.battery_percent, Some(77));

        let (s, _) = parse(Parser::Arctis7ChatMix, &[0x06, 0x24, 0x00, 0xdf, 0, 0, 0, 0]);
        assert_eq!(s.chatmix, Some(ChatMix { game: 100, chat: 50 }));
        let (s, _) = parse(Parser::Arctis7ChatMix, &[0x06, 0x24, 0xbf, 0x00]);
        assert_eq!(s.chatmix, Some(ChatMix { game: 0, chat: 100 }));
        let (s, _) = parse(Parser::Arctis7ChatMix, &[0x06, 0x24, 0xff, 0xff]);
        assert_eq!(s.chatmix, Some(ChatMix { game: 100, chat: 100 }));
    }

    #[test]
    fn arctis9_reads_everything_from_one_answer() {
        let response = [0x00, 0x20, 0x00, 0x7f, 0x01, 0, 0, 0, 0, 19, 0, 0];
        let (s, _) = parse(Parser::Arctis9, &response);
        assert_eq!(s.battery_percent, Some(50));
        assert_eq!(s.charging, Some(true));
        assert_eq!(s.wireless_connected, Some(true));
        assert_eq!(s.chatmix, Some(ChatMix { game: 100, chat: 0 }));

        let offline = [0x00, 0x20, 0x00, 0x7f, 0xff, 0, 0, 0, 0, 10, 10, 0];
        let (s, _) = parse(Parser::Arctis9, &offline);
        assert_eq!(s.wireless_connected, Some(false));
        assert_eq!(s.battery_percent, None);
        assert_eq!(s.chatmix, Some(ChatMix { game: 52, chat: 52 }));
    }

    #[test]
    fn pro_wireless_stops_when_offline() {
        let (s, outcome) = parse(Parser::ProWirelessLink, &[0x02, 0x00]);
        assert_eq!(outcome, Parsed::Stop);
        assert_eq!(s.wireless_connected, Some(false));

        let (s, outcome) = parse(Parser::ProWirelessLink, &[0x04, 0x00]);
        assert_eq!(outcome, Parsed::Continue);
        assert_eq!(s.wireless_connected, Some(true));

        let (s, _) = parse(Parser::ProWirelessBattery, &[0x03]);
        assert_eq!(s.battery_percent, Some(75));
    }

    #[test]
    fn nova_pro_wireless_reads_steps_and_state() {
        let mut response = padded(&[0x07, 0xb0]);
        response[6] = 4;
        response[15] = 0x08;
        let (s, _) = parse(Parser::NovaProWireless, &response);
        assert_eq!(s.battery_percent, Some(50));
        assert_eq!(s.charging, Some(false));
        assert_eq!(s.wireless_connected, Some(true));

        response[15] = 0x02;
        let (s, _) = parse(Parser::NovaProWireless, &response);
        assert_eq!(s.charging, Some(true));

        response[15] = 0x01;
        let (s, outcome) = parse(Parser::NovaProWireless, &response);
        assert_eq!(outcome, Parsed::Stop);
        assert_eq!(s.battery_percent, None);
    }

    #[test]
    fn nova_pro_chatmix_event() {
        let event = padded(&[0x07, 0x45, 80, 30]);
        assert_eq!(
            parse_event(StatusProtocol::NovaProWireless, &event),
            Some(ChatMix { game: 80, chat: 30 })
        );
        assert_eq!(
            parse_event(StatusProtocol::NovaProWireless, &padded(&[0x07, 0x25, 5])),
            None
        );
        assert_eq!(parse_event(StatusProtocol::Nova5, &event), None);
        assert!(StatusProtocol::NovaProWireless.is_event(&event));
        assert!(!StatusProtocol::NovaProWireless.is_event(&padded(&[0x07, 0xb0])));
        assert!(!StatusProtocol::Nova5.is_event(&event));
    }

    #[test]
    fn arctis7_plus_reads_steps_and_chatmix() {
        let (s, _) = parse(Parser::Arctis7Plus, &padded(&[0xb0, 0x03, 3, 0x01, 100, 40]));
        assert_eq!(s.battery_percent, Some(75));
        assert_eq!(s.charging, Some(true));
        assert_eq!(s.wireless_connected, Some(true));
        assert_eq!(s.chatmix, Some(ChatMix { game: 100, chat: 40 }));

        let (s, _) = parse(Parser::Arctis7Plus, &padded(&[0xb0, 0x01, 3, 0x00, 64, 100]));
        assert_eq!(s.wireless_connected, Some(false));
        assert_eq!(s.battery_percent, None);
        assert_eq!(s.chatmix, Some(ChatMix { game: 64, chat: 100 }));
    }

    #[test]
    fn nova3p_and_nova5_read_percent() {
        let (s, _) = parse(Parser::Nova3PWireless, &padded(&[0xb0, 0x03, 0x00, 88, 0x01, 20, 30]));
        assert_eq!(s.battery_percent, Some(88));
        assert_eq!(s.charging, Some(true));
        assert_eq!(s.chatmix, None);

        let (s, _) = parse(Parser::Nova5, &padded(&[0xb0, 0x03, 0x00, 61, 0x00, 100, 35]));
        assert_eq!(s.battery_percent, Some(61));
        assert_eq!(s.charging, Some(false));
        assert_eq!(s.chatmix, Some(ChatMix { game: 100, chat: 35 }));

        let (s, _) = parse(Parser::Nova5, &padded(&[0xb0, 0x02, 0x00, 61, 0x00, 100, 100]));
        assert_eq!(s.wireless_connected, Some(false));
        assert_eq!(s.battery_percent, None);
        assert!(s.chatmix.is_some());
    }

    #[test]
    fn nova7_discrete_and_percent_battery() {
        let discrete = Parser::Nova7 {
            battery: BatteryScale::Steps(4),
            chatmix: true,
        };
        let (s, _) = parse(discrete, &padded(&[0xb0, 0x00, 4, 0x03, 100, 72]));
        assert_eq!(s.battery_percent, Some(100));
        assert_eq!(s.charging, Some(false));
        assert_eq!(s.wireless_connected, Some(true));
        assert_eq!(s.chatmix, Some(ChatMix { game: 100, chat: 72 }));

        let percent = Parser::Nova7 {
            battery: BatteryScale::Percent,
            chatmix: false,
        };
        let (s, _) = parse(percent, &padded(&[0xb0, 0x00, 57, 0x02, 100, 72]));
        assert_eq!(s.battery_percent, Some(57));
        assert_eq!(s.charging, Some(true));
        assert_eq!(s.chatmix, None);

        let (s, _) = parse(percent, &padded(&[0xb0, 0x00, 57, 0x00]));
        assert_eq!(s.wireless_connected, Some(false));
        assert_eq!(s.battery_percent, None);
    }

    #[test]
    fn nova7_sidetone_read() {
        let (s, _) = parse(Parser::Nova7Sidetone, &padded(&[0x20, 0x00, 0x02]));
        assert_eq!(s.extra.get("sidetone").map(String::as_str), Some("2"));
        let (s, _) = parse(Parser::Nova7Sidetone, &padded(&[0x20, 0x00, 0x09]));
        assert!(s.extra.is_empty());
    }

    #[test]
    fn gamebuds_reports_lowest_active_bud() {
        let (s, _) = parse(Parser::GameBuds, &padded(&[0xb0, 0, 0, 0x03, 0x03, 80, 64]));
        assert_eq!(s.battery_percent, Some(64));
        assert_eq!(s.wireless_connected, Some(true));
        assert_eq!(s.extra.get("battery_left").map(String::as_str), Some("80"));
        assert_eq!(s.extra.get("battery_right").map(String::as_str), Some("64"));

        let (s, _) = parse(Parser::GameBuds, &padded(&[0xb0, 0, 0, 0x03, 0x02, 80, 0]));
        assert_eq!(s.battery_percent, Some(80));
        assert!(!s.extra.contains_key("battery_right"));

        let (s, _) = parse(Parser::GameBuds, &padded(&[0xb0, 0, 0, 0x02, 0x02, 0, 0]));
        assert_eq!(s.battery_percent, None);
        assert_eq!(s.wireless_connected, Some(false));
    }

    #[test]
    fn short_responses_leave_status_untouched() {
        for parser in [
            Parser::Arctis1,
            Parser::Arctis9,
            Parser::NovaProWireless,
            Parser::Arctis7Plus,
            Parser::Nova5,
            Parser::Nova7 {
                battery: BatteryScale::Percent,
                chatmix: true,
            },
            Parser::GameBuds,
        ] {
            let (s, outcome) = parse(parser, &[0xb0]);
            assert!(s.is_empty(), "{parser:?} filled fields from a 1-byte answer");
            assert_eq!(outcome, Parsed::Continue);
        }
    }

    #[test]
    fn queries_match_reference_requests() {
        let q = StatusProtocol::Nova5.queries();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].request.bytes, vec![0x00, 0xb0]);
        assert_eq!(q[0].accept, Accept::Prefer { index: 0, value: 0xb0 });

        let q = StatusProtocol::ProWireless.queries();
        assert_eq!(q.len(), 2);
        assert_eq!(&q[0].request.bytes[..2], &[0x41, 0xaa]);
        assert_eq!(q[0].request.bytes.len(), 31);
        assert_eq!(&q[1].request.bytes[..2], &[0x40, 0xaa]);

        let q = StatusProtocol::Arctis7 { battery: false }.queries();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].request.bytes, vec![0x06, 0x24]);

        let q = StatusProtocol::NovaProWireless.queries();
        assert_eq!(&q[0].request.bytes[..2], &[0x06, 0xb0]);
        assert_eq!(q[0].request.bytes.len(), 31);

        let q = StatusProtocol::Nova7 {
            battery: BatteryScale::Percent,
            chatmix: true,
            sidetone_read: true,
        }
        .queries();
        assert_eq!(q.len(), 2);
        assert_eq!(&q[1].request.bytes[..2], &[0x00, 0x20]);
        assert_eq!(q[1].request.bytes.len(), 64);
        assert_eq!(q[1].accept, Accept::Only { index: 0, value: 0x20 });
    }

    #[test]
    fn accept_rules() {
        assert!(Accept::First.matches(&[1]));
        let prefer = Accept::Prefer { index: 0, value: 0xb0 };
        assert!(prefer.matches(&[0xb0]));
        assert!(!prefer.matches(&[0x20]));
        assert!(prefer.allows_fallback());
        assert!(!Accept::Only { index: 0, value: 0x20 }.allows_fallback());
    }
}
