// Copyright (c) 2026 vivo Mobile Communication Co., Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::devices::{Device, DeviceClass, DeviceId, DeviceManager};
use alloc::{format, string::String, sync::Arc, vec, vec::Vec};
use blueos_infra::tinyrwlock::RwLock;
use embedded_io::ErrorKind;
use libc::{FBIOGET_FSCREENINFO, FBIOGET_VSCREENINFO, FBIOPUT_VSCREENINFO, FBIO_DRAW_AREA};

/// Linux framebuffer character-device major number.
pub const FRAMEBUFFER_MAJOR: usize = 29;

/// Kernel representation of Linux `struct fb_bitfield`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct FramebufferBitfield {
    /// Bit offset from the right.
    pub offset: u32,
    /// Bitfield length in bits.
    pub length: u32,
    /// Non-zero when the most significant bit is on the right.
    pub msb_right: u32,
}

/// Kernel representation of Linux `struct fb_fix_screeninfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct FramebufferFixedInfo {
    /// Identification string.
    pub id: [libc::c_char; 16],
    /// Start of framebuffer memory, represented as BlueOS `c_ulong` (`u32`).
    pub smem_start: u32,
    /// Framebuffer memory length in bytes.
    pub smem_len: u32,
    /// Framebuffer type.
    pub type_: u32,
    /// Interleave information for interleaved planes.
    pub type_aux: u32,
    /// Visual type.
    pub visual: u32,
    /// Zero when horizontal panning is unsupported.
    pub xpanstep: u16,
    /// Zero when vertical panning is unsupported.
    pub ypanstep: u16,
    /// Zero when vertical wrapping is unsupported.
    pub ywrapstep: u16,
    /// Bytes per line.
    pub line_length: u32,
    /// Start of memory-mapped I/O, represented as BlueOS `c_ulong` (`u32`).
    pub mmio_start: u32,
    /// Memory-mapped I/O length in bytes.
    pub mmio_len: u32,
    /// Accelerator identifier.
    pub accel: u32,
    /// Driver capabilities.
    pub capabilities: u16,
    /// Reserved for ABI compatibility.
    pub reserved: [u16; 2],
    /// Tail padding reserved for ABI compatibility.
    pub reserved_tail: [u8; 12],
}

/// Kernel representation of Linux `struct fb_var_screeninfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct FramebufferVariableInfo {
    /// Visible horizontal resolution in pixels.
    pub xres: u32,
    /// Visible vertical resolution in pixels.
    pub yres: u32,
    /// Virtual horizontal resolution in pixels.
    pub xres_virtual: u32,
    /// Virtual vertical resolution in pixels.
    pub yres_virtual: u32,
    /// Horizontal offset from virtual to visible resolution.
    pub xoffset: u32,
    /// Vertical offset from virtual to visible resolution.
    pub yoffset: u32,
    /// Pixel depth in bits.
    pub bits_per_pixel: u32,
    /// Grayscale mode.
    pub grayscale: u32,
    /// Red component bitfield.
    pub red: FramebufferBitfield,
    /// Green component bitfield.
    pub green: FramebufferBitfield,
    /// Blue component bitfield.
    pub blue: FramebufferBitfield,
    /// Transparency component bitfield.
    pub transp: FramebufferBitfield,
    /// Non-standard pixel format marker.
    pub nonstd: u32,
    /// Activation flags.
    pub activate: u32,
    /// Physical screen height in millimeters.
    pub height: u32,
    /// Physical screen width in millimeters.
    pub width: u32,
    /// Acceleration flags.
    pub accel_flags: u32,
    /// Pixel clock in picoseconds.
    pub pixclock: u32,
    /// Left margin in pixels.
    pub left_margin: u32,
    /// Right margin in pixels.
    pub right_margin: u32,
    /// Upper margin in pixels.
    pub upper_margin: u32,
    /// Lower margin in pixels.
    pub lower_margin: u32,
    /// Horizontal sync length in pixels.
    pub hsync_len: u32,
    /// Vertical sync length in pixels.
    pub vsync_len: u32,
    /// Sync flags.
    pub sync: u32,
    /// Video mode flags.
    pub vmode: u32,
    /// Rotation angle.
    pub rotate: u32,
    /// Color space identifier.
    pub colorspace: u32,
    /// Reserved for ABI compatibility.
    pub reserved: [u32; 4],
}

const _: [(); 12] = [(); core::mem::size_of::<FramebufferBitfield>()];
const _: [(); 80] = [(); core::mem::size_of::<FramebufferFixedInfo>()];
const _: [(); 160] = [(); core::mem::size_of::<FramebufferVariableInfo>()];

/// Geometry and source layout for a two-dimensional framebuffer update.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FramebufferDrawArea {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// Source bytes between the start of two consecutive rows.
    pub stride: u32,
}

unsafe fn load_user_variable_info(
    ptr: *const FramebufferVariableInfo,
) -> Result<FramebufferVariableInfo, ErrorKind> {
    if ptr.is_null() {
        return Err(ErrorKind::InvalidInput);
    }
    Ok(*ptr)
}

unsafe fn load_user_draw_area(
    ptr: *const libc::fb_draw_area,
) -> Result<libc::fb_draw_area, ErrorKind> {
    if ptr.is_null() {
        return Err(ErrorKind::InvalidInput);
    }
    Ok(core::ptr::read(ptr))
}

unsafe fn store_user_fixed_info(
    ptr: *mut FramebufferFixedInfo,
    fixed_info: &FramebufferFixedInfo,
) -> Result<(), ErrorKind> {
    if ptr.is_null() {
        return Err(ErrorKind::InvalidInput);
    }
    *ptr = *fixed_info;
    Ok(())
}

unsafe fn store_user_variable_info(
    ptr: *mut FramebufferVariableInfo,
    variable_info: &FramebufferVariableInfo,
) -> Result<(), ErrorKind> {
    if ptr.is_null() {
        return Err(ErrorKind::InvalidInput);
    }
    *ptr = *variable_info;
    Ok(())
}

/// Driver-facing framebuffer operations.
pub trait FramebufferOps {
    /// Return fixed framebuffer metadata.
    fn fixed_info(&self) -> Result<FramebufferFixedInfo, ErrorKind>;

    /// Return variable framebuffer metadata.
    fn variable_info(&self) -> Result<FramebufferVariableInfo, ErrorKind>;

    /// Validate and apply a variable-info update, returning the effective state.
    fn set_variable_info(
        &mut self,
        variable_info: &FramebufferVariableInfo,
    ) -> Result<FramebufferVariableInfo, ErrorKind>;

    /// Read framebuffer bytes starting at `offset`.
    fn read_bytes(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, ErrorKind>;

    /// Write framebuffer bytes starting at `offset`.
    fn write_bytes(&mut self, offset: u64, buf: &[u8]) -> Result<usize, ErrorKind>;

    /// Draw a two-dimensional pixel area from a source buffer.
    fn draw_area(&mut self, area: FramebufferDrawArea, pixels: &[u8]) -> Result<(), ErrorKind>;

    /// Return the framebuffer byte length.
    fn byte_len(&self) -> Result<u64, ErrorKind>;
}

/// Character-device wrapper for a framebuffer implementation.
pub struct FramebufferDevice<T: FramebufferOps> {
    name: String,
    id: DeviceId,
    ops: Arc<RwLock<T>>,
}

unsafe impl<T: FramebufferOps> Send for FramebufferDevice<T> {}
unsafe impl<T: FramebufferOps> Sync for FramebufferDevice<T> {}

impl<T: FramebufferOps + 'static> FramebufferDevice<T> {
    /// Create a framebuffer device named `fb{index}` with Linux framebuffer major `29`.
    #[must_use]
    pub fn new(index: usize, ops: Arc<RwLock<T>>) -> Self {
        Self::with_id(
            format!("fb{index}"),
            DeviceId::new(FRAMEBUFFER_MAJOR, index),
            ops,
        )
    }

    /// Create a framebuffer device with an explicit name and device id.
    #[must_use]
    pub fn with_id(name: String, id: DeviceId, ops: Arc<RwLock<T>>) -> Self {
        Self { name, id, ops }
    }

    /// Register a framebuffer device named `fb{index}`.
    pub fn register(index: usize, ops: Arc<RwLock<T>>) -> Result<(), ErrorKind> {
        Self::register_device(Arc::new(Self::new(index, ops)))
    }

    /// Register an already constructed framebuffer device.
    pub fn register_device(device: Arc<Self>) -> Result<(), ErrorKind> {
        let name = device.name.clone();
        log::debug!("Registering framebuffer device: {}", name);
        DeviceManager::get().register_device(name, device)
    }

    /// Return the device name.
    #[must_use]
    pub fn device_name(&self) -> &str {
        &self.name
    }

    /// Return the device identifier.
    #[must_use]
    pub const fn device_id(&self) -> DeviceId {
        self.id
    }

    /// Return fixed framebuffer metadata.
    pub fn fixed_info(&self) -> Result<FramebufferFixedInfo, ErrorKind> {
        self.ops.read().fixed_info()
    }

    /// Return variable framebuffer metadata.
    pub fn variable_info(&self) -> Result<FramebufferVariableInfo, ErrorKind> {
        self.ops.read().variable_info()
    }

    /// Validate and apply a variable-info update, returning the effective state.
    pub fn set_variable_info(
        &self,
        variable_info: &FramebufferVariableInfo,
    ) -> Result<FramebufferVariableInfo, ErrorKind> {
        self.ops.write().set_variable_info(variable_info)
    }

    fn draw_area(&self, request: &libc::fb_draw_area) -> Result<(), ErrorKind> {
        if request.pixels.is_null() || request.width == 0 || request.height == 0 {
            return Err(ErrorKind::InvalidInput);
        }

        let mut ops = self.ops.write();
        let info = ops.variable_info()?;
        if info.bits_per_pixel == 0 || info.bits_per_pixel % 8 != 0 {
            return Err(ErrorKind::Unsupported);
        }

        let bytes_per_pixel = info.bits_per_pixel / 8;
        let end_x = request
            .x
            .checked_add(request.width)
            .ok_or(ErrorKind::InvalidInput)?;
        let end_y = request
            .y
            .checked_add(request.height)
            .ok_or(ErrorKind::InvalidInput)?;
        if end_x > info.xres || end_y > info.yres {
            return Err(ErrorKind::InvalidInput);
        }

        let row_bytes = request
            .width
            .checked_mul(bytes_per_pixel)
            .ok_or(ErrorKind::InvalidInput)?;
        if request.stride < row_bytes || request.stride % bytes_per_pixel != 0 {
            return Err(ErrorKind::InvalidInput);
        }
        let source_len = request
            .height
            .checked_sub(1)
            .and_then(|rows| rows.checked_mul(request.stride))
            .and_then(|prefix| prefix.checked_add(row_bytes))
            .and_then(|len| usize::try_from(len).ok())
            .ok_or(ErrorKind::InvalidInput)?;
        let pixels =
            unsafe { core::slice::from_raw_parts(request.pixels.cast::<u8>(), source_len) };

        ops.draw_area(
            FramebufferDrawArea {
                x: request.x,
                y: request.y,
                width: request.width,
                height: request.height,
                stride: request.stride,
            },
            pixels,
        )
    }
}

impl<T: FramebufferOps + 'static> Device for FramebufferDevice<T> {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn class(&self) -> DeviceClass {
        DeviceClass::Char
    }

    fn id(&self) -> DeviceId {
        self.id
    }

    fn read(&self, pos: u64, buf: &mut [u8], _is_nonblocking: bool) -> Result<usize, ErrorKind> {
        if buf.is_empty() {
            return Ok(0);
        }

        let byte_len = self.ops.read().byte_len()?;
        if pos >= byte_len {
            return Ok(0);
        }

        let remaining = byte_len - pos;
        let read_len =
            usize::try_from(remaining).map_or(buf.len(), |remaining| remaining.min(buf.len()));
        self.ops.write().read_bytes(pos, &mut buf[..read_len])
    }

    fn write(&self, pos: u64, buf: &[u8], _is_nonblocking: bool) -> Result<usize, ErrorKind> {
        if buf.is_empty() {
            return Ok(0);
        }

        let byte_len = self.ops.read().byte_len()?;
        if pos >= byte_len {
            return Ok(0);
        }

        let remaining = byte_len - pos;
        let write_len =
            usize::try_from(remaining).map_or(buf.len(), |remaining| remaining.min(buf.len()));
        self.ops.write().write_bytes(pos, &buf[..write_len])
    }

    fn ioctl(&self, request: u32, arg: usize) -> Result<(), ErrorKind> {
        match request {
            req if req == FBIOGET_FSCREENINFO => {
                let fixed_info = self.ops.read().fixed_info()?;
                unsafe { store_user_fixed_info(arg as *mut FramebufferFixedInfo, &fixed_info) }
            }
            req if req == FBIOGET_VSCREENINFO => {
                let variable_info = self.ops.read().variable_info()?;
                unsafe {
                    store_user_variable_info(arg as *mut FramebufferVariableInfo, &variable_info)
                }
            }
            req if req == FBIOPUT_VSCREENINFO => {
                let requested_info =
                    unsafe { load_user_variable_info(arg as *const FramebufferVariableInfo)? };
                let effective_info = self.ops.write().set_variable_info(&requested_info)?;
                unsafe {
                    store_user_variable_info(arg as *mut FramebufferVariableInfo, &effective_info)
                }
            }
            req if req == FBIO_DRAW_AREA => {
                let request = unsafe { load_user_draw_area(arg as *const libc::fb_draw_area)? };
                self.draw_area(&request)
            }
            _ => Err(ErrorKind::Unsupported),
        }
    }

    fn capacity(&self) -> Result<u64, ErrorKind> {
        self.ops.read().byte_len()
    }
}
