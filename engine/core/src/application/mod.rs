// SPDX-License-Identifier: GPL-3.0-or-later
//! Portable cSC4App owner from application.md sections 1-15.
//!
//! See README.md for section coverage and the spec's remaining open boundaries.
//! Foreign objects remain traits; no native ABI layout or plugin is installed.
//! All getters use plain shared state, with no RefCell or borrowed COM releases.

pub mod city;
pub mod command_line;
pub mod compatibility;
pub mod directories;
mod lifecycle;
pub mod preference_application;
#[cfg(test)]
mod resource_tests;
pub mod resources;
pub mod save_path;
#[cfg(test)]
mod settled_tests;
pub mod ui;
#[cfg(test)]
mod ui_tests;
pub mod video;

pub mod directors;
pub mod archive;

use crate::commands::parse_delimited_text;
use crate::gz_messages::Token;

pub use command_line::{fix_startup_resolution, is_startup_resolution_ok, Preferences};
pub use directories::{Directories, UserFolder};
pub use lifecycle::{Director, Flow, Helper, Lifecycle, Object, Service, SERVICES};

pub const PRE_APP_INIT: u32 = 0x28E9_397A;
pub const POST_APP_INIT: u32 = 0x28E9_397B;
pub const PRE_APP_SHUTDOWN: u32 = 0x28E9_397C;
pub const POST_APP_SHUTDOWN: u32 = 0x28E9_397D;
pub const QUIT_REQUEST: u32 = 0xC7AA_61C6;
pub const GAME_IDLE: u32 = 0xBA44_EB51;
pub const GP_CHEAT: u32 = 0x2AB5_C431;
pub const EXCEPTION_CHEAT: u32 = 0x2B60_E45B;
pub const FORCE_IDLE_CHEAT: u32 = 0x8B6D_4F8D;

/// Exact unpublished/assigned boundary. No boolean default means "implemented".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Boundary {
    BuildVersion,
    ResolveUserDataDirectory,
    RestartWait,
    FindAndPossiblyFixIniFiles,
    CursorAndOsSettings,
    PluginDateChecks,
    AddCodePlugins(Vec<u8>),
    OpenLogs,
    FindAndRegisterLanguages,
    PreferencesChanged,
    DriverConfiguration,
    SetupResources,
    LanguageManagerHookup,
    RenderPropertyVector,
    SlOnAppInit1,
    InitAlphaSystem,
    FindTestScript(Vec<u8>),
    SavePreferences,
    ShutdownMessageServer1,
    ShutdownHelperOrder,
    QuitService,
    CloseCity,
    RequestQuit,
    CityObliteration,
    ResolutionAcceptance { width: u32, height: u32, depth: u32 },
    UserDirectoryAcceptance(Vec<u8>),
    InvalidNativeInput(&'static str),
}

/// Opaque host failures have no inferred fallback/rollback (AP6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostFailure(pub &'static str);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    Send,
    /// Always MessagePost(message, false), never directed delivery.
    Post,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageData {
    App(Token),
    Word(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Message {
    pub kind: u32,
    pub data: MessageData,
}

/// Primitive external calls only. Missing app-owned algorithms are Boundary,
/// never effects. Tokens returned by create carry one owned reference; query
/// tokens are borrowed. Registration takes its own framework reference.
pub trait Host {
    fn effect(&mut self, effect: Effect);
    fn create(&mut self, object: Object) -> Result<Token, HostFailure>;
    fn register_service(&mut self, service: Service, object: Token) -> Result<(), HostFailure>;
    fn release_created(&mut self, object: Token);
    fn install_directory(&mut self) -> Option<Vec<u8>>;
    fn graphics_driver(&mut self) -> Option<Token>;
    fn check_graphics_driver(&mut self, driver: Token) -> bool;
    fn directory_exists(&mut self, path: &[u8]) -> bool;
    fn watcher_running(&mut self, watcher: Token) -> bool;
    fn watched_directory(&mut self, watcher: Token) -> Vec<u8>;
    fn install_exception_filter(&mut self) -> Token;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    ReadStartupOption(&'static str),
    AddDirector(Director),
    FetchFrameworkManagers,
    SetRendererDriver(Token),
    RegisterV1 {
        kind: u32,
        target: Token,
    },
    RegisterV2 {
        kinds: [u32; 9],
        target: Token,
    },
    UnregisterV2 {
        kinds: [u32; 9],
        target: Token,
    },
    UnregisterV1 {
        kind: u32,
        target: Token,
    },
    UnregisterCheat {
        manager: Token,
        id: u32,
    },
    RegisterCheat {
        manager: Token,
        name: &'static str,
        id: u32,
    },
    InitTutorialRegistry(Token),
    Message {
        message: Message,
        delivery: Delivery,
    },
    Warning {
        title: &'static str,
        text: &'static str,
    },
    LaunchExceptionReportUploader,
    SetEnvironment {
        name: &'static str,
        value: Vec<u8>,
    },
    CreateDirectory(Vec<u8>),
    StartWatcher {
        watcher: Token,
        path: Vec<u8>,
    },
    StopWatcher(Token),
    ExecuteLuaScript(Vec<u8>),
    NetworkCommandGeneratorEnabled,
    Pause {
        paused: bool,
        flags: [bool; 4],
    },
    SleepCurrentThread(u32),
    ResourceManagerSlotAc(Token),
    RaiseTestException(Vec<u8>),
    DestroyCurrentScreen(Token),
    FrameworkQuit(u32),
    QuitServiceSlot44,
    ProcessPriority(u32),
    CpuCount(i32),
    RenderProperty {
        key: u32,
        value: u32,
    },
    ExceptionOption {
        index: u8,
        value: bool,
    },
    RestoreExceptionFilter(Token),
    Pump {
        generation: u8,
        args: [u32; 3],
    },
}

/// Elapsed milliseconds are explicit state, so tests can exercise the rare
/// threshold branch without replacing the application's timer algorithm.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timers {
    pub gate: u32,
    pub activity: u32,
    pub input: u32,
    pub autosave: u32,
    pub periodic: u32,
}

impl Timers {
    pub fn advance(&mut self, milliseconds: u32) {
        self.gate = self.gate.wrapping_add(milliseconds);
        self.activity = self.activity.wrapping_add(milliseconds);
        self.input = self.input.wrapping_add(milliseconds);
        self.autosave = self.autosave.wrapping_add(milliseconds);
        self.periodic = self.periodic.wrapping_add(milliseconds);
    }
}

/// Portable application state. `new` is a neutral composition state, not an
/// implementation of the native constructor. Only mode/quitting constructor
/// values are published. AP6: the integrating owner must supply preferences,
/// paths, timer periods/thresholds, existing flags and borrowed service tokens;
/// no native defaults or service-discovery algorithm are claimed here.
#[derive(Debug)]
pub struct Application {
    pub preferences: Preferences,
    pub directories: Directories,
    pub timers: Timers,
    pub autosave_period: u32,
    pub periodic_period: u32,
    pub idle_threshold: u32,
    pub idle_subtract: u32,
    pub message_server2_available: bool,
    pub resource_manager: Option<Token>,
    pub watchers: [Option<Token>; 2],
    pub screen_owner: Option<Token>,
    pub field_314: Option<Token>,
    pub city_state: city::CityState,
    pub preferences_image: compatibility::PreferencesImage,
    pub resource_registry: resources::Registry,
    pub startup_plugin_directories: Option<Vec<Vec<u8>>>,
    pub(crate) instance_policy: (bool, bool),
    pub handles: compatibility::AppHandles,
    pub app_name: Vec<u8>,
    pub mode: u32,
    pub debug_enabled: bool,
    pub popup_dialogs_enabled: bool,
    pub driver_id: u32,
    pub exception_enabled: bool,
    pub exception_verbose: bool,
    pub(crate) previous_exception_filter: Option<Token>,
    app_token: Token,
    pause_on_focus_loss: bool,
    fully_paused: bool,
    quitting: bool,
    pub(crate) lifecycle: lifecycle::Runner,
}

impl Application {
    pub fn new(app_token: Token, preferences: Preferences, directories: Directories) -> Self {
        Self {
            preferences,
            directories,
            timers: Timers::default(),
            autosave_period: 0,
            periodic_period: 0,
            idle_threshold: 0,
            idle_subtract: 0,
            message_server2_available: false,
            resource_manager: None,
            watchers: [None; 2],
            screen_owner: None,
            field_314: None,
            city_state: city::CityState::default(),
            preferences_image: compatibility::PreferencesImage::default(),
            resource_registry: resources::Registry::default(),
            startup_plugin_directories: None,
            instance_policy: (false, false),
            handles: compatibility::AppHandles::default(),
            app_name: Vec::new(),
            mode: 0,
            debug_enabled: false,
            popup_dialogs_enabled: false,
            driver_id: 0,
            exception_enabled: false,
            exception_verbose: false,
            previous_exception_filter: None,
            app_token,
            pause_on_focus_loss: false,
            fully_paused: false,
            quitting: false,
            lifecycle: lifecycle::Runner::default(),
        }
    }

    pub fn fully_paused(&self) -> bool {
        self.fully_paused
    }
    pub fn pause_on_focus_loss(&self) -> bool {
        self.pause_on_focus_loss
    }
    pub fn quitting(&self) -> bool {
        self.quitting
    }

    /// Empty app name leaves caller output untouched (section 2).
    pub fn app_file_name(&self, extension: &[u8], out: &mut Vec<u8>) {
        if !self.app_name.is_empty() {
            out.clone_from(&self.app_name);
            out.extend_from_slice(extension);
        }
    }

    pub fn enable_full_game_pause_on_focus_loss(&mut self, enabled: bool) {
        self.preferences.pause_on_focus_loss = enabled;
        self.preferences_image.0[0x4A4] = u8::from(enabled);
        self.pause_on_focus_loss = enabled;
    }

    pub fn on_app_activate(&mut self, active: bool, host: &mut impl Host) {
        if self.pause_on_focus_loss {
            self.full_game_pause(!active, host);
        }
    }

    pub fn full_game_pause(&mut self, paused: bool, host: &mut impl Host) {
        if paused != self.fully_paused {
            self.fully_paused = paused;
            host.effect(Effect::Pause {
                paused,
                flags: [true; 4],
            });
        }
    }

    pub fn post_sc4_app_message(&self, kind: u32, post: bool, host: &mut impl Host) {
        self.deliver(kind, MessageData::App(self.app_token), post, host);
    }

    fn deliver(&self, kind: u32, data: MessageData, post: bool, host: &mut impl Host) {
        if self.message_server2_available {
            host.effect(Effect::Message {
                message: Message { kind, data },
                delivery: if post { Delivery::Post } else { Delivery::Send },
            });
        }
    }

    pub fn run_message_server_pump(&self, generation: u8, args: [u32; 3], host: &mut impl Host) {
        // Section 2 specifies forwarding only; pump internals belong to gz_messages.
        host.effect(Effect::Pump { generation, args });
    }

    /// v2 always acknowledges; returned boundaries are actions still owed by
    /// the caller. In particular a region/city event never starts those owners.
    pub fn message_v2(&mut self, kind: u32, host: &mut impl Host) -> Result<(bool, Option<Boundary>), HostFailure> {
        let boundary = match kind {
            0x26D3_1EC2 | 0x8BB5_BB46 => {
                self.mode = 0;
                None
            }
            0xCBB5_BB45 => {
                self.mode = 1;
                None
            }
            0xEA8A_E29A => {
                self.mode = 2;
                None
            }
            0x231B_BF91 => Some(Boundary::PreferencesChanged),
            0x29A8_9B05 => {
                self.begin_quit()?;
                None
            }
            0x0B62_7AD3 | 0x49A7_80B2 => {
                host.effect(Effect::LaunchExceptionReportUploader);
                None
            }
            0xEA65_224E => Some(Boundary::CityObliteration),
            GAME_IDLE => {
                if let Some(manager) = self.resource_manager {
                    host.effect(Effect::ResourceManagerSlotAc(manager));
                }
                None
            }
            0xFA0C_6390 | 0x7A10_4750 => {
                self.timers.input = 0;
                None
            }
            _ => None,
        };
        Ok((true, boundary))
    }

    pub fn message_v1(
        &mut self,
        kind: u32,
        data1: Token,
        text: &[u8],
        host: &mut impl Host,
    ) -> Result<(bool, Option<Boundary>), HostFailure> {
        let boundary = match kind {
            QUIT_REQUEST => Some(Boundary::RequestQuit),
            0x8716_3754 if self.watchers.contains(&Some(data1)) => {
                let path = host.watched_directory(data1);
                // Completion must post 0xA42396C6 after loading, not now.
                self.begin_plugin_change(path)?;
                None
            }
            0x230E_27AC => {
                let (_, words) = parse_delimited_text(text, b' ', usize::MAX);
                match data1 as u32 {
                    GP_CHEAT if words.len() == 1 => {
                        // AP9: exact on/off only; unlike command-line switches,
                        // section 6.1 does not list enabled/disabled synonyms.
                        match words[0].to_ascii_lowercase().as_slice() {
                            b"on" => self.enable_full_game_pause_on_focus_loss(true),
                            b"off" => self.enable_full_game_pause_on_focus_loss(false),
                            _ => return Ok((true, None)),
                        }
                        // "apply it" has no published active-state lookup (AP9).
                        return Ok((true, Some(Boundary::PreferencesChanged)));
                    }
                    EXCEPTION_CHEAT if words.len() == 1 => host.effect(Effect::RaiseTestException(words[0].clone())),
                    FORCE_IDLE_CHEAT => self.deliver(GAME_IDLE, MessageData::Word(60), true, host),
                    _ => {}
                }
                None
            }
            _ => None,
        };
        Ok((true, boundary))
    }

    /// Starts the idle continuation; drive with run(). SavePreferences is an
    /// unresolved app method, and blocks before the later periodic step.
    pub fn begin_idle(&mut self) -> Result<(), HostFailure> {
        self.start_idle()
    }
}

#[cfg(test)]
mod owner_tests;
#[cfg(test)]
mod tests;
