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

//! BlueOS wireless UAPI —— 内核与用户态之间的"无线"ABI 契约 crate。
//!
//! 本 crate 汇集所有无线相关协议在 syscall 边界上交换数据所用的类型与常量,
//! 由内核与用户态共同依赖,杜绝内存布局漂移。它是 [`blueos_header`](crate)
//! (通用 syscall UAPI)的兄弟 crate,仅聚焦"无线"子系统。
//!
//! # 与主流内核的对应
//!
//! | 关注点 | Linux | NuttX | BlueOS(本 crate) |
//! |--------|-------|-------|--------------------|
//! | 无线 UAPI 契约 | `include/uapi/linux/wireless.h` | `include/nuttx/wireless/wireless.h` | `kernel/wireless/` |
//!
//! # 子模块组织(可扩展)
//!
//! - [`wext`]:Wireless Extensions(wext)框架。`iwreq` / `SIOCGIW*` 等,
//!   WiFi 扫描/连接走的就是这套(源自 Linux wext,与 NuttX 同源)。
//!
//! 以后蓝牙 / ieee802154 等无线协议的 UAPI 归入各自的子模块
//! (如 `bluetooth`、`ieee802154`),与 Linux / NuttX 的组织方式保持一致。

// 正常构建为 no_std(供内核使用);`--test` 构建切到 std,以使用标准测试框架跑单测。
#![cfg_attr(not(test), no_std)]

pub mod wext;

// 便捷再导出:用户可直接 `use blueos_wireless::Iwreq;`
pub use wext::*;
