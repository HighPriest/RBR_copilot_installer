//! Localization: the strings live in per-language STRINGTABLEs in
//! `res/installer.rc`, and choosing a language is `SetThreadUILanguage`.
//! The `IDS_*` ids below match the `#define`s there.

mod language;
mod string_table;
mod wide_string;

pub use language::{os_language, set_language, Language};
pub use string_table::{instance, load_string, load_string_arg};
pub use wide_string::to_wide;

pub const IDS_ALREADY_RUNNING: u16 = 11;
pub const IDS_APP_TITLE: u16 = 20;

pub const IDS_BTN_CONTINUE: u16 = 30;
pub const IDS_BTN_ACCEPT: u16 = 31;
pub const IDS_BTN_NEXT: u16 = 32;
pub const IDS_BTN_DOWNLOAD: u16 = 33;
pub const IDS_BTN_INSTALL: u16 = 34;
pub const IDS_BTN_FINISH: u16 = 35;
pub const IDS_BTN_BROWSE: u16 = 36;
pub const IDS_CANCEL_CONFIRM: u16 = 37;
pub const IDS_BTN_BACK: u16 = 38;
pub const IDS_BTN_CANCEL: u16 = 39;

pub const IDS_S1_TITLE: u16 = 50;
pub const IDS_S1_PROMPT: u16 = 51;
pub const IDS_S1_RESTART: u16 = 52;

pub const IDS_S2_TITLE: u16 = 60;
pub const IDS_S2_SECRET: u16 = 61;
pub const IDS_S2_CHECKING: u16 = 62;
pub const IDS_S2_ONLINE: u16 = 63;
pub const IDS_S2_OFFLINE: u16 = 64;
pub const IDS_S2_OFFLINE_MOTD: u16 = 65;
pub const IDS_S2_BADSECRET: u16 = 66;

pub const IDS_S3_TITLE: u16 = 70;
pub const IDS_S3_PATH: u16 = 71;
pub const IDS_S3_GAME: u16 = 72;
pub const IDS_S3_DETECTING: u16 = 73;
pub const IDS_S3_NOGAME: u16 = 74;

pub const IDS_S4_TITLE: u16 = 80;
pub const IDS_S4_LOADING: u16 = 81;
pub const IDS_S4_OFFLINE: u16 = 82;
pub const IDS_S4_EMPTY: u16 = 83;
pub const IDS_S4_READY: u16 = 84;
pub const IDS_S4_MISSING: u16 = 85;
pub const IDS_S4_FETCH_FAILED: u16 = 86;
pub const IDS_STATE_DOWNLOADED: u16 = 90;
pub const IDS_STATE_UPDATE: u16 = 91;
pub const IDS_STATE_LOCAL: u16 = 92;
pub const IDS_STATE_INSTALLED: u16 = 93;
pub const IDS_STATE_UNAVAILABLE: u16 = 94;

pub const IDS_S5_TITLE: u16 = 100;
pub const IDS_S5_LANGUAGE: u16 = 101;
pub const IDS_S5_VOICE: u16 = 102;
pub const IDS_S5_STYLE: u16 = 103;
pub const IDS_S5_MISSING: u16 = 104;
pub const IDS_S5_NONE: u16 = 105;

pub const IDS_S6_TITLE: u16 = 110;
pub const IDS_S6_DOWNLOADING: u16 = 111;
pub const IDS_S6_INSTALLING: u16 = 113;
pub const IDS_S6_FINISHED: u16 = 114;
pub const IDS_S6_NOTHING: u16 = 115;
pub const IDS_S6_CANCELLING: u16 = 116;

pub const IDS_S7_TITLE: u16 = 130;
pub const IDS_S7_FAILED: u16 = 132;
pub const IDS_S7_CANCELLED: u16 = 133;
pub const IDS_S7_PENDING_ENGINE: u16 = 134;
pub const IDS_S7_GAME: u16 = 135;
pub const IDS_S7_DOWNLOADS: u16 = 136;
pub const IDS_S7_EXTRAS: u16 = 137;
pub const IDS_S7_LOGFILE: u16 = 138;
pub const IDS_S7_NOTHING_TODO: u16 = 139;
