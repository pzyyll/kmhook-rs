//! Copyright: 2024 Lizc. All rights reserved.
//! License: MIT License
//! You may obtain a copy of the License at https://opensource.org/licenses/MIT
//!
//! Author: Lizc
//! Created Data: 2024-09-29
//!
//! Description: This is a windows event listener library.
#![allow(incomplete_features)]

pub mod listener;
pub mod types_ext;

// #[cfg(all(feature = "Fake", not(feature = "DLL")))]
#[path = "event_loop_fake.rs"]
mod event_loop;


pub(crate) mod worker;

// pub trait KeyIdFrom {
//     fn from_win(scancode: u32, vkcode: u32) -> std::result::Result<Self, ()>
//     where
//         Self: Sized;
// }

pub(crate) const WM_USER_RECHECK_HOOK: u32 = 1;
