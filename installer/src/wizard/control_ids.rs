//! Dialog and control ids, matching `res/installer.rc`.

/// The scene-title static, the same id in every page template.
pub const IDC_SCENE_TITLE: u16 = 299;

pub const IDD_S1_LANGUAGE: u16 = 300;
pub const IDC_S1_PROMPT: u16 = 301;
pub const IDC_S1_COMBO: u16 = 302;
pub const IDC_S1_NOTE: u16 = 303;

pub const IDD_S2_MOTD: u16 = 310;
pub const IDC_S2_MOTD: u16 = 311;
pub const IDC_S2_SECRET_LABEL: u16 = 312;
pub const IDC_S2_SECRET: u16 = 313;
pub const IDC_S2_STATUS: u16 = 314;

pub const IDD_S3_FOLDER: u16 = 320;
pub const IDC_S3_PATH_LABEL: u16 = 321;
pub const IDC_S3_PATH: u16 = 322;
pub const IDC_S3_BROWSE: u16 = 323;
pub const IDC_S3_GAME_LABEL: u16 = 324;
pub const IDC_S3_GAME: u16 = 325;
pub const IDC_S3_DOWNLOAD: u16 = 326;
pub const IDC_S3_CONFIGURE: u16 = 327;
pub const IDC_S3_ROUTES: u16 = 328;
pub const IDC_S3_RESTORE: u16 = 329;

pub const IDD_S4_DOWNLOAD: u16 = 330;
pub const IDC_S4_TREE: u16 = 331;
pub const IDC_S4_STATUS: u16 = 332;

/// Scene 4b reuses scene 6's control ids, since one set of handlers drives both pages.
pub const IDD_S4B_FETCH: u16 = 335;

pub const IDD_S5_CONFIG: u16 = 340;
pub const IDC_S5_LANG_LABEL: u16 = 341;
pub const IDC_S5_LANG: u16 = 342;
pub const IDC_S5_VOICE_LABEL: u16 = 343;
pub const IDC_S5_VOICE: u16 = 344;
pub const IDC_S5_STYLE_LABEL: u16 = 345;
pub const IDC_S5_STYLE: u16 = 346;

pub const IDD_S6_INSTALL: u16 = 350;
pub const IDC_S6_PHASE: u16 = 351;
pub const IDC_S6_PROGRESS: u16 = 352;
pub const IDC_S6_ITEM: u16 = 353;
pub const IDC_S6_LOG: u16 = 354;

pub const IDD_S7_SUMMARY: u16 = 360;
pub const IDC_S7_RESULT: u16 = 361;
pub const IDC_S7_DETAIL: u16 = 362;
pub const IDC_S7_LOG: u16 = 363;
