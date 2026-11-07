//! Private Rubik's Cube engine.
//!
//! The implementation is derived from RCuber 0.7.20 and retained under its MIT license.
//! See THIRD_PARTY_NOTICES.md for the full rcuber, Kewb, PyCuber, and min2phase notices.

// Some upstream helpers remain useful for parity tests and future engine work even when the
// current public API does not call them directly. First-party code remains under strict lints.
#![allow(dead_code, clippy::all)]

pub(crate) mod cfop;
pub(crate) mod constants;
pub(crate) mod cubie;
pub(crate) mod error;
pub(crate) mod facelet;
pub(crate) mod min2phase;
pub(crate) mod moves;
