// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline adapters for the owner codecs in automata §§2–6. No Init, Lua,
//! animation ticks or controller registration runs during a save round trip.

use super::{
    AutomataManager, Automaton, Controller, ControllerManager, ManagerKind, NoControllerHost, ScriptSystem,
    CLSID_CONTROLLER_MANAGER, CLSID_SCRIPT_SYSTEM,
};
use crate::stream::{InStream, OutStream, SaveObject};
use std::rc::Rc;

pub enum AutomataRecord {
    Script(Box<ScriptSystem>),
    Controllers(Box<ControllerManager>),
    Manager(Box<AutomataManager>),
    Object(Rc<dyn SaveObject>),
}

impl AutomataRecord {
    pub fn create(class: u32) -> Option<Self> {
        if class == CLSID_SCRIPT_SYSTEM {
            return Some(Self::Script(Box::new(ScriptSystem::new())));
        }
        if class == CLSID_CONTROLLER_MANAGER {
            return Some(Self::Controllers(Box::new(ControllerManager::new(Box::new(
                NoControllerHost,
            )))));
        }
        if let Some(kind) = ManagerKind::from_class(class) {
            return Some(Self::Manager(Box::new(AutomataManager::new(kind))));
        }
        if let Some(controller) = Controller::for_class(class) {
            return Some(Self::Object(controller));
        }
        Automaton::for_class(class).map(|object| Self::Object(object))
    }

    pub fn read(&mut self, r: &mut dyn InStream) -> bool {
        match self {
            Self::Script(value) => value.read(r),
            Self::Controllers(value) => value.read(r),
            Self::Manager(value) => value.read(r),
            Self::Object(value) => value.read(r),
        }
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        match self {
            Self::Script(value) => value.write(w),
            Self::Controllers(value) => value.write(w),
            Self::Manager(value) => value.write(w),
            Self::Object(value) => value.write(w),
        }
    }
}
