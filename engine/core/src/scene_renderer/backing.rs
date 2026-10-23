// SPDX-License-Identifier: GPL-3.0-or-later
//! Process-wide backing-store ownership and render3d §12 algorithms.
use super::{Result, Unsupported};
use std::sync::{Arc, Mutex};

pub trait BufferDriver: Send {
    fn create(&mut self, kind: u32) -> Option<u32>;
    fn free(&mut self, handle: u32);
    fn viewport_size(&mut self) -> (i32, i32);
    fn save(&mut self, handle: u32, args: [i32; 6]) -> bool;
    fn restore(&mut self, handle: u32, args: [i32; 6]) -> bool;
}

/// All renderers in a portable process explicitly share this Arc. The state
/// owns D and both created handles; destruction frees handles before dropping D.
/// A native D adapter would own its acquired service ref. No native service is
/// installed/switched by this module; no unsynchronized process globals exist.
pub type SharedBackingStore<D> = Arc<Mutex<BackingStore<D>>>;
pub struct BackingStore<D: BufferDriver> {
    driver: Option<D>,
    buffers: [Option<u32>; 2],
    enabled: bool,
}
impl<D: BufferDriver> BackingStore<D> {
    pub fn new(driver: Option<D>) -> Self {
        // Portable explicit initial switch, not an unpublished native default.
        Self {
            driver,
            buffers: [None; 2],
            enabled: false,
        }
    }
    pub fn shared(self) -> SharedBackingStore<D> {
        Arc::new(Mutex::new(self))
    }
    pub fn handles(&self) -> [Option<u32>; 2] {
        self.buffers
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled && self.buffers[0].is_some();
    }
    pub fn driver(&self) -> Option<&D> {
        self.driver.as_ref()
    }
    pub fn driver_mut(&mut self) -> Option<&mut D> {
        self.driver.as_mut()
    }
    pub fn create(&mut self) -> bool {
        let Some(driver) = self.driver.as_mut() else {
            return false;
        };
        for i in 0..2 {
            if self.buffers[i].is_none() {
                self.buffers[i] = driver.create(i as u32).filter(|h| *h != 0);
            }
        }
        let [Some(a), Some(b)] = self.buffers else {
            self.destroy();
            return false;
        };
        let (w, h) = driver.viewport_size();
        let args = [0, 0, w, h, 0, 0];
        // Short circuit at FIRST failed probe, not after probing both buffers.
        if !(driver.save(a, args) && driver.restore(a, args) && driver.save(b, args) && driver.restore(b, args)) {
            self.destroy();
            return false;
        }
        true
    }
    pub fn destroy(&mut self) -> bool {
        let Some(driver) = self.driver.as_mut() else {
            return false;
        };
        for buffer in &mut self.buffers {
            if let Some(handle) = buffer.take() {
                driver.free(handle);
            }
        }
        true
    }
    pub fn save(
        &mut self,
        rect: [i32; 4],
        viewport: (i32, i32),
        shake: (i32, i32),
        partial_allowed: bool,
    ) -> Result<bool> {
        let [x, y, mut w, mut h] = rect;
        let (mut src_x, mut src_y, mut dst_x, mut dst_y) = (x, y, x, y);
        if shake != (0, 0) {
            src_x = sub(x, shake.0)?;
            if src_x < 0 {
                dst_x = add(x, shake.0)?;
                w = sub(w, shake.0)?;
                src_x = 0;
                if w <= 0 {
                    return Ok(true);
                }
            } else if add(src_x, w)? > viewport.0 {
                w = sub(viewport.0, src_x)?;
                if w <= 0 {
                    return Ok(true);
                }
            }
            src_y = sub(y, shake.1)?;
            if src_y < 0 {
                dst_y = add(y, shake.1)?;
                h = sub(h, shake.1)?;
                src_y = 0;
                if h <= 0 {
                    return Ok(true);
                }
            } else if add(src_y, h)? > viewport.1 {
                h = sub(viewport.1, src_y)?;
                if h <= 0 {
                    return Ok(true);
                }
            }
        }
        if w <= 0 || h <= 0 {
            return Ok(false);
        }
        let (Some(driver), [Some(a), Some(b)]) = (&mut self.driver, self.buffers) else {
            return Ok(false);
        };
        if !partial_allowed && (w, h) != viewport {
            return Ok(false);
        }
        let args = [src_x, src_y, w, h, dst_x, dst_y];
        let _ = driver.save(a, args);
        let _ = driver.save(b, args);
        Ok(true)
    }
    pub fn restore_rect(&mut self, args: [i32; 6]) -> bool {
        let (Some(driver), [Some(a), Some(b)]) = (&mut self.driver, self.buffers) else {
            return false;
        };
        let _ = driver.restore(a, args);
        let _ = driver.restore(b, args);
        true
    }
    pub fn restore(&mut self, viewport: (i32, i32), shake: (i32, i32)) -> Result<bool> {
        let ax = shake.0.checked_abs().ok_or(Unsupported::Overflow)?;
        let ay = shake.1.checked_abs().ok_or(Unsupported::Overflow)?;
        Ok(self.restore_rect([
            if shake.0 < 0 { ax } else { 0 },
            if shake.1 < 0 { ay } else { 0 },
            sub(viewport.0, ax)?,
            sub(viewport.1, ay)?,
            shake.0.max(0),
            shake.1.max(0),
        ]))
    }
}
impl<D: BufferDriver> Drop for BackingStore<D> {
    fn drop(&mut self) {
        self.destroy();
    }
}
fn add(a: i32, b: i32) -> Result<i32> {
    a.checked_add(b).ok_or(Unsupported::Overflow)
}
fn sub(a: i32, b: i32) -> Result<i32> {
    a.checked_sub(b).ok_or(Unsupported::Overflow)
}
