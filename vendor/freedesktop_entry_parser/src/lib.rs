/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! A library for parsing FreeDesktop entry files. These files are used in the
//! [Desktop Entry
//! files](https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html),
//! [Icon Theme index
//! files](https://specifications.freedesktop.org/icon-theme-spec/icon-theme-spec-latest.html),
//! and [Systemd unit
//! files](https://www.freedesktop.org/software/systemd/man/systemd.unit.html).
//! They are similar to ini files but are distinct enough that an ini parse
//! would not work.
//!
//! This library slightly breaks from the FreeDesktop spec by allowing duplicate sections
//! and keys because some common users like systemd use duplicate keys. This is why attribute
//! values are lists. Duplicate sections are merged.
//!
//! This library does join continued lines. An attribute value line ending in a backslash (\) will
//! have the content of the next line starting at the first non-whitespace character merged into
//! one string. It does not modify attribute values in any other way, like replacing escape
//! sequences like `\r` and `\n` with their respective real values.
//!
//! # Struct of Freedesktop Entry Files
//!
//! Freedesktop entry files are split up into section, each with a header in the
//! form `[NAME]`. Each section has attributes, which are key value pairs,
//! separated by and `=`.  Some attributes have parameters.  These are values
//! between `[]` and the end of the attribute name.  These are often use for
//! localization.
//!
//! Here is a snippet from `firefox.desktop`
//!
//! ```ignore
//! [Desktop Entry]
//! Version=1.0
//! Name=Firefox
//! GenericName=Web Browser
//! GenericName[ar]=متصفح ويب
//! GenericName[ast]=Restolador Web
//! GenericName[bn]=ওয়েব ব্রাউজার
//! GenericName[ca]=Navegador web
//! Exec=/usr/lib/firefox/firefox %u
//! Icon=firefox
//!
//! [Desktop Action new-window]
//! Name=New Window
//! Name[ach]=Dirica manyen
//! Name[af]=Nuwe venster
//! Name[an]=Nueva finestra
//! Exec=/usr/lib/firefox/firefox --new-window %u
//! ```
//!
//! The first section is called `Desktop Entry`.  Is has many attributes
//! including `Name` which is `Firefox`.  The `GenericName` attributes has a
//! param. The default value is in English but there are also values with a
//! parameter for different locales.
//!
//! # APIs
//!
//! This library has two APIs, a high level API and a low level byte oriented API.
//! The main entry point for the high level API is [`Entry`] and the entry point for
//! the lower level API is the [`low_level::parse_entry`] function.
//!
//! ## High Level API
//!
//! As example input lets use the contents of `sshd.service`
//! ```ignore
//! [Unit]
//! Description=OpenSSH Daemon
//! Wants=sshdgenkeys.service
//! After=sshdgenkeys.service
//! After=network.target
//!
//! [Service]
//! ExecStart=/usr/bin/sshd -D
//! ExecReload=/bin/kill -HUP $MAINPID
//! KillMode=process
//! Restart=always
//!
//! [Install]
//! WantedBy=multi-user.target
//! ```
//!
//! For example, to print the start command we could do this:
//! ```
//! # #[cfg(miri)] fn main() {}
//! # #[cfg(not(miri))]
//! # fn main() -> Result<(), std::io::Error> {
//! use freedesktop_entry_parser::parse_entry;
//!
//! let entry = parse_entry("./test_data/sshd.service")?;
//! let start_cmd = entry
//!     .section("Service")
//!     .expect("Section Service doesn't exist")
//!     .attr("ExecStart")
//!     .get(0)
//!     .expect("Attribute ExecStart doesn't exist");
//! println!("{}", start_cmd);
//!
//! # Ok::<(), std::io::Error>(())
//! # }
//! ```
//! ## Low Level API
//!
//! The [`low_level`] module exposes a less ergonomic but is a *mostly* zero-copy API that is used
//! by the higher level API in this module. You may want to use this API if you need to squeeze
//! out all of the performance you can.

#![forbid(unsafe_code)]

mod errors;
pub mod low_level;
#[cfg(feature = "high_level")]
mod high_level;
#[cfg(feature = "high_level")]
pub use high_level::*;

