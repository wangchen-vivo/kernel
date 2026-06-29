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

//! Wireless Extensions(wext)框架的 UAPI 定义。
//!
//! 对应 C 头 `linux/wireless.h`(Linux:`include/uapi/linux/wireless.h`,
//! NuttX:`include/nuttx/wireless/wireless.h`)。本模块是它的 Rust 移植:
//! `struct iwreq` / `union iwreq_data` / `SIOCGIW*` 等,供 WiFi 扫描、连接等
//! 通过 `ioctl(socket_fd, SIOCGIW*, &iwreq)` 在用户态↔内核间交换数据。
//!
//! 类型命名遵循 Rust 惯例(CamelCase);与 C 名的对应见各结构体注释
//! (如 [`Iwreq`] = `struct iwreq`)。

// 类型别名 `__u8` / `__s16` / `__u32` 等刻意保留 Linux 内核的 typedef 命名
// (便于与 C 头 `linux/wireless.h` 对照、grep),故放宽 non_camel_case_types
// 检查(libc crate 对 FFI 类型同样这么处理)。
#![allow(non_camel_case_types)]

use libc::{c_char, c_int, c_uint, c_ulong, c_uchar, c_short, c_ushort, c_void, sockaddr};

// ---- 定长类型别名(对应内核 `__u8` / `__s32` 等)----
pub type __u8 = c_uchar;
pub type __u16 = c_ushort;
pub type __s16 = c_short;
pub type __u32 = c_uint;
pub type __s32 = c_int;

// ---- 基础常量 ----
pub const IFNAMSIZ: usize = 16;
pub const IW_ESSID_MAX_SIZE: usize = 32;
pub const IW_MAX_FREQUENCIES: usize = 32;
pub const IW_SCAN_MAX_DATA: usize = 4096;

// ---- Wireless ioctl 命令号(linux/wireless.h)----
// ★ 这些值必须与内核 `NetIfaceControl::from_raw_ioctl` 的 match 分支一一对应,
//   改一处要同步两边(本 crate 即为双方的单一真相源)。
pub const SIOCSIWCOMMIT: c_ulong = 0x8B00;
pub const SIOCGIWNAME: c_ulong = 0x8B01;
pub const SIOCSIWNWID: c_ulong = 0x8B02;
pub const SIOCGIWNWID: c_ulong = 0x8B03;
pub const SIOCSIWFREQ: c_ulong = 0x8B04;
pub const SIOCGIWFREQ: c_ulong = 0x8B05;
pub const SIOCSIWMODE: c_ulong = 0x8B06;
pub const SIOCGIWMODE: c_ulong = 0x8B07;
pub const SIOCSIWSENS: c_ulong = 0x8B08;
pub const SIOCGIWSENS: c_ulong = 0x8B09;
pub const SIOCSIWAP: c_ulong = 0x8B14;
pub const SIOCGIWAP: c_ulong = 0x8B15;
pub const SIOCSIWMLME: c_ulong = 0x8B16;
pub const SIOCGIWMLME: c_ulong = 0x8B17;
pub const SIOCSIWSCAN: c_ulong = 0x8B18; // 触发扫描
pub const SIOCGIWSCAN: c_ulong = 0x8B19; // 读取扫描结果
pub const SIOCSIWESSID: c_ulong = 0x8B1A; // 设置 SSID(连接)
pub const SIOCGIWESSID: c_ulong = 0x8B1B; // 读取当前 SSID
pub const SIOCSIWENCODE: c_ulong = 0x8B2A; // 设置密钥/密码(连接)
pub const SIOCGIWENCODE: c_ulong = 0x8B2B;
pub const SIOCSIWAUTH: c_ulong = 0x8B32;
pub const SIOCGIWAUTH: c_ulong = 0x8B33;

// ---- 扫描相关标志 ----
pub const IW_SCAN_DEFAULT: c_uint = 0x0000;
pub const IW_SCAN_ALL_ESSID: c_uint = 0x0001;
pub const IW_SCAN_THIS_ESSID: c_uint = 0x0002;
pub const IW_SCAN_ALL_FREQ: c_uint = 0x0004;
pub const IW_SCAN_THIS_FREQ: c_uint = 0x0008;
pub const IW_SCAN_ALL_MODE: c_uint = 0x0010;
pub const IW_SCAN_THIS_MODE: c_uint = 0x0020;
pub const IW_SCAN_ALL_RATE: c_uint = 0x0040;
pub const IW_SCAN_THIS_RATE: c_uint = 0x0080;
pub const IW_SCAN_TYPE_ACTIVE: c_int = 0;
pub const IW_SCAN_TYPE_PASSIVE: c_int = 1;

// ---- 结构体(#[repr(C)] 保证与 C / linux 逐字节一致)----

/// C: `struct iw_param`
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct IwParam {
    pub value: __s32,
    pub fixed: __u8,
    pub disabled: __u8,
    pub flags: __u16,
}

/// C: `struct iw_point`
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IwPoint {
    pub pointer: *mut c_void,
    pub length: __u16,
    pub flags: __u16,
}

/// C: `struct iw_freq`
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct IwFreq {
    pub m: __s32,
    pub e: __s16,
    pub i: __u8,
    pub flags: __u8,
}

/// C: `struct iw_quality`
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct IwQuality {
    pub qual: __u8,
    pub level: __u8,
    pub noise: __u8,
    pub updated: __u8,
}

/// C: `union iwreq_data` —— [`Iwreq`] 的第二个字;不同 ioctl 复用同一块内存。
///
/// 读 / 写 union 的某个变体需要 `unsafe`(编译器无法知道当前哪个变体有效)。
#[repr(C)]
#[derive(Clone, Copy)]
pub union IwreqData {
    pub name: [c_char; IFNAMSIZ],
    pub nwid: IwParam,
    pub essid: IwPoint, // SIOCGIWESSID / SIOCSIWESSID
    pub freq: IwFreq,
    pub sens: IwParam,
    pub bitrate: IwParam,
    pub txpower: IwParam,
    pub rts: IwParam,
    pub frag: IwParam,
    pub mode: __u32,
    pub retry: IwParam,
    pub encoding: IwPoint,
    pub power: IwParam,
    pub qual: IwQuality,
    pub ap_addr: sockaddr,
    pub addr: sockaddr,
    pub param: IwParam,
    pub data: IwPoint, // SIOCGIWSCAN / SIOCSIWSCAN:pointer 指向缓冲区
}

/// C: `struct iwreq` —— wireless ioctl 的主参数(对应 `ifreq`)。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Iwreq {
    /// 接口名,如 `"wlan0"`。对应 C union 里唯一的 `ifrn_name` 字段。
    pub ifr_ifrn: [c_char; IFNAMSIZ],
    pub u: IwreqData,
}

/// C: `struct iw_scan_req`
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IwScanReq {
    pub scan_type: __u8,
    pub essid_len: __u8,
    pub num_channels: __u8,
    pub flags: __u8,
    pub bssid: sockaddr,
    pub essid: [__u8; IW_ESSID_MAX_SIZE],
    pub min_channel_time: __u32,
    pub max_channel_time: __u32,
    pub channel_list: [IwFreq; IW_MAX_FREQUENCIES],
}

// ---- 编译期 ABI 自检:命令号 / 常量写错 → build 直接失败(无需跑测试)----
const _: () = assert!(SIOCSIWSCAN == 0x8B18);
const _: () = assert!(SIOCGIWSCAN == 0x8B19);
const _: () = assert!(SIOCSIWESSID == 0x8B1A);
const _: () = assert!(SIOCGIWESSID == 0x8B1B);
const _: () = assert!(SIOCSIWENCODE == 0x8B2A);
const _: () = assert!(IFNAMSIZ == 16);
const _: () = assert!(IW_ESSID_MAX_SIZE == 32);

// ---- 单元测试:`--test` 构建时启用,在开发机(host)上跑 ----
#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{offset_of, size_of};

    /// 这些值是 Linux wext ABI 的硬性规定,错一个 wifi 命令就对不上内核。
    #[test]
    fn cmd_numbers_match_linux_wireless_h() {
        assert_eq!(SIOCSIWSCAN, 0x8B18);
        assert_eq!(SIOCGIWSCAN, 0x8B19);
        assert_eq!(SIOCSIWESSID, 0x8B1A);
        assert_eq!(SIOCGIWESSID, 0x8B1B);
        assert_eq!(SIOCSIWENCODE, 0x8B2A);
        // SET/GET 成对出现,命令号差 1 —— 防漏号 / 重号
        assert_eq!(SIOCGIWSCAN - SIOCSIWSCAN, 1);
        assert_eq!(SIOCGIWESSID - SIOCSIWESSID, 1);
        assert_eq!(SIOCGIWAP - SIOCSIWAP, 1);
    }

    #[test]
    fn size_constants_match_linux() {
        assert_eq!(IFNAMSIZ, 16);
        assert_eq!(IW_ESSID_MAX_SIZE, 32);
        assert_eq!(IW_MAX_FREQUENCIES, 32);
        assert_eq!(IW_SCAN_MAX_DATA, 4096);
    }

    /// `struct iwreq = ifrn_name[16] + union iwreq_data[16] = 32` 字节。
    /// 关键:32/64 位上都得是 32(64 位 host 与 32 位 target 已对齐验证)。
    #[test]
    fn iwreq_layout_matches_c_struct() {
        assert_eq!(size_of::<Iwreq>(), 32, "Iwreq 必须 32 字节(16+16)");
        assert_eq!(offset_of!(Iwreq, ifr_ifrn), 0);
        assert_eq!(offset_of!(Iwreq, u), IFNAMSIZ, "u 紧跟接口名之后 @offset 16");
    }

    /// union 大小 = 最大成员;32 / 64 位都是 16。
    #[test]
    fn iwreq_data_union_is_16_bytes() {
        assert_eq!(size_of::<IwreqData>(), 16);
        assert!(size_of::<IwreqData>() >= size_of::<[c_char; IFNAMSIZ]>());
    }

    #[test]
    fn sockaddr_is_16_bytes() {
        assert_eq!(size_of::<sockaddr>(), 16);
    }

    /// ★ 最关键:把 `Iwreq` 当"裸字节缓冲"读写字段,再 memcpy 出去、读回来。
    /// 这正是 ioctl 在用户态↔内核之间传数据的方式。
    /// 通过 = `#[repr(C)]` 生效、字段顺序稳定、可安全 memcpy。
    #[test]
    fn iwreq_roundtrips_through_byte_buffer() {
        let mut req: Iwreq = unsafe { core::mem::zeroed() };
        // 写接口名 "wlan0"(ifr_ifrn 是 [c_char; IFNAMSIZ],c_char == i8)
        let name: [c_char; 5] = [
            b'w' as c_char, b'l' as c_char, b'a' as c_char, b'n' as c_char, b'0' as c_char,
        ];
        req.ifr_ifrn[..name.len()].copy_from_slice(&name);
        // 写 u.data:直接构造整个 union 变体(初始化 union 是 safe 的,无需 unsafe)
        let mut buf = [0u8; 8];
        req.u = IwreqData {
            data: IwPoint {
                pointer: buf.as_mut_ptr() as *mut c_void,
                length: buf.len() as u16,
                flags: 0,
            },
        };

        // 序列化到字节缓冲(模拟跨 syscall 拷贝)
        let mut bytes = [0u8; 64];
        assert!(size_of::<Iwreq>() <= bytes.len());
        unsafe {
            core::ptr::copy_nonoverlapping(
                &req as *const Iwreq as *const u8,
                bytes.as_mut_ptr(),
                size_of::<Iwreq>(),
            );
        }

        // 反序列化回来,字段必须完全一致
        let back: Iwreq = unsafe { core::ptr::read(bytes.as_ptr() as *const Iwreq) };
        assert_eq!(&back.ifr_ifrn[..name.len()], &name[..]);
        unsafe {
            assert_eq!(back.u.data.length, buf.len() as u16);
            assert_eq!(back.u.data.pointer, req.u.data.pointer);
        }
    }
}
