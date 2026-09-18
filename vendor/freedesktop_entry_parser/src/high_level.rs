/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use indexmap::{map::Iter, Equivalent, IndexMap};
use std::fs::File;
use std::io::{self, Read};
use std::iter::FusedIterator;
use std::ops::Index;
use std::path::Path;

pub use crate::errors::ParseError;

/// Parse a FreeDesktop entry file.
///
/// # Example
///
/// ```
/// use freedesktop_entry_parser::parse_entry;
///
/// let entry = parse_entry("./test_data/sshd.service")?;
/// let start_cmd = &entry
///     .get("Service", "ExecStart")
///     .expect("File doesn't have Service -> ExecStart")[0];
/// assert_eq!(start_cmd, "/usr/bin/sshd -D");
/// # Ok::<(), std::io::Error>(())
/// ```
///
/// # Errors
///
/// If there is a parse error it'll be return an [`ParseError`] wrapped [`std::io::Error`]
/// with the [`std::io::ErrorKind::Other`].
///
/// ```
/// use freedesktop_entry_parser::parse_entry;
///
/// match parse_entry("./test_data/sshd.service") {
///     Ok(entry) => println!("{entry:#?}"),
///     Err(e) if e.kind() == std::io::ErrorKind::Other => println!("Parse error {e:?}"),
///     Err(e) => println!("I/O Error {e:?}"),
/// }
/// ```
pub fn parse_entry(input: impl AsRef<Path>) -> io::Result<Entry> {
    Entry::parse_file(input)
}

/// A parsed FreeDesktop entry file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry(IndexMap<String, Section>);

impl<'a> IntoIterator for &'a Entry {
    type Item = (&'a String, &'a Section);

    type IntoIter = SectionIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.sections()
    }
}

impl<Idx: AsRef<str>> Index<Idx> for Entry {
    type Output = Section;

    /// Returns a reference to the section corresponding to the supplied title.
    ///
    /// # Panics
    ///
    /// Panics if the title is not a section in the `Entry`.
    fn index(&self, title: Idx) -> &Self::Output {
        self.section(title).expect("no section found for title")
    }
}

/// A section of a FreeDesktop entry file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section(IndexMap<AttrKey, Vec<String>>);

impl<'a> IntoIterator for &'a Section {
    type Item = (&'a AttrKey, &'a Vec<String>);

    type IntoIter = AttrIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.attrs()
    }
}

impl<Idx: AsRef<str>> Index<Idx> for Section {
    type Output = [String];

    /// Returns a reference to the value corresponding to the supplied key.
    ///
    /// # Panics
    ///
    /// Panics if the key is not in the `Section`.
    fn index(&self, key: Idx) -> &Self::Output {
        self.attr(key)
    }
}

/// The key type returned by [`AttrIter`] that contains the attribute's key and optional parameter.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AttrKey {
    pub key: String,
    pub param: Option<String>,
}

/// A reference version for [`AttrKey`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AttrKeyRef<'a> {
    pub key: &'a str,
    pub param: Option<&'a str>,
}

impl Entry {
    /// Parse an entry from byte buffer.
    ///
    /// Unlike [`Entry::parse_file`] this function performs no I/O and just parses the
    /// input string or bytes.
    ///
    /// # Example
    ///
    /// ```
    /// use freedesktop_entry_parser::Entry;
    ///
    /// let entry_bytes = b"
    /// [Unit]
    /// Description=OpenSSH Daemon
    /// Wants=sshdgenkeys.service
    /// After=sshdgenkeys.service
    /// After=network.target
    ///
    /// [Service]
    /// ExecStart=/usr/bin/sshd -D
    /// ExecReload=/bin/kill -HUP $MAINPID
    /// KillMode=process
    /// Restart=always
    ///
    /// [Install]
    /// WantedBy=multi-user.target
    /// ";
    /// let entry = Entry::parse(entry_bytes)?;
    /// let start_cmd = &entry
    ///     .get("Service", "ExecStart")
    ///     .expect("File doesn't have Service -> ExecStart")[0];
    /// assert_eq!(start_cmd, "/usr/bin/sshd -D");
    /// # Ok::<(), freedesktop_entry_parser::ParseError>(())
    /// ```
    pub fn parse(input: impl AsRef<[u8]>) -> Result<Self, ParseError> {
        let mut sections_map = IndexMap::new();
        for section in crate::low_level::parse_entry_str(input.as_ref()) {
            let section = section?;
            let attr_map = sections_map
                .entry(section.title.to_owned())
                .or_insert_with(|| Section(IndexMap::new()));
            for attr in section.attrs {
                let key = match attr.param {
                    Some(param) => AttrKey {
                        key: param.attr_name.to_owned(),
                        param: Some(param.param.to_owned()),
                    },
                    None => AttrKey {
                        key: attr.name.to_owned(),
                        param: None,
                    },
                };
                attr_map
                    .0
                    .entry(key)
                    .and_modify(|attr_list: &mut Vec<String>| {
                        attr_list.push(attr.value.clone().into())
                    })
                    .or_insert(vec![attr.value.into()]);
            }
        }
        Ok(Entry(sections_map))
    }

    /// Parse entry from file.
    ///
    /// If there is a parse error it'll be return an [`ParseError`] wrapped [`std::io::Error`]
    /// with the [`std::io::ErrorKind::Other`]. Equivalent to [`parse_entry`].
    pub fn parse_file(path: impl AsRef<Path>) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        Self::parse(buf).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    /// Get section with `title`.
    ///
    /// # Example
    ///
    /// ```
    /// use freedesktop_entry_parser::Entry;
    ///
    /// let file_content = "
    /// [Desktop Entry]
    /// Name=Firefox
    /// ";
    /// let entry = Entry::parse(file_content)?;
    /// let section = entry.section("Desktop Entry");
    /// println!("{:#?}", section);
    /// # Ok::<(), freedesktop_entry_parser::ParseError>(())
    /// ```
    pub fn section(&self, title: impl AsRef<str>) -> Option<&Section> {
        self.0.get(title.as_ref())
    }

    /// Check if the entry has a section `title`.
    ///
    /// # Example
    ///
    /// ```
    /// use freedesktop_entry_parser::Entry;
    ///
    /// let file_content = "
    /// [Desktop Entry]
    /// Name=Firefox
    /// ";
    /// let entry = Entry::parse(file_content)?;
    /// assert!(entry.has_section("Desktop Entry"));
    /// # Ok::<(), freedesktop_entry_parser::ParseError>(())
    /// ```
    pub fn has_section(&self, title: impl AsRef<str>) -> bool {
        self.0.contains_key(title.as_ref())
    }

    /// Iterator over sections.
    ///
    /// # Example
    ///
    /// ```
    /// use freedesktop_entry_parser::Entry;
    ///
    /// let file_content = "
    /// [Unit]
    /// Description=OpenSSH Daemon
    /// Wants=sshdgenkeys.service
    ///
    /// [Service]
    /// ExecStart=/usr/bin/sshd -D
    ///
    /// [Install]
    /// WantedBy=multi-user.target
    /// ";
    /// let entry = Entry::parse(file_content)?;
    /// for section in entry.sections() {
    ///     println!("{:#?}", section);
    /// }
    /// # Ok::<(), freedesktop_entry_parser::ParseError>(())
    /// ```
    pub fn sections(&self) -> SectionIter<'_> {
        SectionIter(self.0.iter())
    }

    /// Get the values of `attr` in `section`.
    ///
    /// If the section doesn't exist, this will return `None`.
    /// Otherwise, if the attribute is defined multiple times, all of them will be in the list,
    /// if the attribute is missing the list will be empty.
    ///
    /// # Example
    ///
    /// ```
    /// use freedesktop_entry_parser::Entry;
    ///
    /// let file_content = "
    /// [Desktop Entry]
    /// Name=Firefox
    /// ";
    /// let entry = Entry::parse(file_content)?;
    /// let value = entry.get("Desktop Entry", "Name");
    /// println!("{:#?}", value);
    /// # Ok::<(), freedesktop_entry_parser::ParseError>(())
    /// ```
    ///
    /// This is a convince method that's equivalent to calling:
    /// ```
    /// use freedesktop_entry_parser::Entry;
    ///
    /// let file_content = "
    /// [Desktop Entry]
    /// Name=Firefox
    /// ";
    /// let entry = Entry::parse(file_content)?;
    /// let value = match entry.section("Desktop Entry") {
    ///     Some(section) => Some(section.attr("Name")),
    ///     None => None,
    /// };
    /// println!("{:#?}", value);
    /// # Ok::<(), freedesktop_entry_parser::ParseError>(())
    /// ```
    pub fn get(
        &self,
        section: impl AsRef<str>,
        attr: impl AsRef<str>,
    ) -> Option<&[String]> {
        Some(self.section(section)?.attr(attr))
    }

    /// Get the values of `attr` with `param` in `section`.
    ///
    /// If the section doesn't exist, this will return `None`.
    /// Otherwise, if the attribute is defined multiple times, all of them will be in the list,
    /// if the attribute is missing the list will be empty.
    ///
    /// # Example
    ///
    /// ```
    /// use freedesktop_entry_parser::Entry;
    ///
    /// let file_content = "
    /// [Desktop Entry]
    /// Name=Firefox
    /// GenericName[ca]=Navegador web
    /// ";
    /// let entry = Entry::parse(file_content)?;
    /// let value = entry.get_with_param("Desktop Entry", "GenericName", "ca");
    /// println!("{:#?}", value);
    /// # Ok::<(), freedesktop_entry_parser::ParseError>(())
    /// ```
    ///
    /// This is a convince method that's equivalent to calling:
    /// ```
    /// use freedesktop_entry_parser::Entry;
    ///
    /// let file_content = "
    /// [Desktop Entry]
    /// Name=Firefox
    /// GenericName[ca]=Navegador web
    /// ";
    /// let entry = Entry::parse(file_content)?;
    /// let value = match entry.section("Desktop Entry") {
    ///     Some(section) => Some(section.attr_with_param("GenericName", "ca")),
    ///     None => None,
    /// };
    /// println!("{:#?}", value);
    /// # Ok::<(), freedesktop_entry_parser::ParseError>(())
    /// ```
    pub fn get_with_param(
        &self,
        section: impl AsRef<str>,
        attr: impl AsRef<str>,
        param: impl AsRef<str>,
    ) -> Option<&[String]> {
        Some(self.section(section)?.attr_with_param(attr, param))
    }
}

static EMPTY_ATTRS: &[String] = &[];

impl Section {
    /// Get the values of the attribute `key`.
    ///
    /// If the attribute is defined multiple times, all of them will be in the list,
    /// if the attribute is missing the list will be empty.
    pub fn attr(&self, key: impl AsRef<str>) -> &[String] {
        let key = AttrKeyRef {
            key: key.as_ref(),
            param: None,
        };
        self.0.get(&key).map(Vec::as_slice).unwrap_or(EMPTY_ATTRS)
    }

    /// Get the value of the attribute `name` and param value `param_val`.
    ///
    /// If the attribute is defined multiple times, all of them will be in the list,
    /// if the attribute is missing the list will be empty.
    pub fn attr_with_param(
        &self,
        key: impl AsRef<str>,
        param: impl AsRef<str>,
    ) -> &[String] {
        let key = AttrKeyRef {
            key: key.as_ref(),
            param: Some(param.as_ref()),
        };
        self.0.get(&key).map(Vec::as_slice).unwrap_or(EMPTY_ATTRS)
    }

    /// Check if this section has an attribute with `name`.
    pub fn has_attr(&self, key: impl AsRef<str>) -> bool {
        let key = AttrKeyRef {
            key: key.as_ref(),
            param: None,
        };
        self.0.contains_key(&key)
    }

    /// Check if this section has an attribute with `name` and param value `param_val`.
    pub fn has_attr_with_param(
        &self,
        key: impl AsRef<str>,
        param: impl AsRef<str>,
    ) -> bool {
        let key = AttrKeyRef {
            key: key.as_ref(),
            param: Some(param.as_ref()),
        };
        self.0.contains_key(&key)
    }

    /// Iterator over attributes in this section
    pub fn attrs(&self) -> AttrIter<'_> {
        AttrIter(self.0.iter())
    }
}

impl<'a> Equivalent<AttrKey> for AttrKeyRef<'a> {
    fn equivalent(&self, key: &AttrKey) -> bool {
        self.key == key.key && self.param == key.param.as_deref()
    }
}

/// Iterator over the sections in an entry.
///
/// Created from [`Entry::sections`].
pub struct SectionIter<'a>(Iter<'a, String, Section>);

impl<'a> Iterator for SectionIter<'a> {
    type Item = (&'a String, &'a Section);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
    fn count(self) -> usize {
        self.0.count()
    }
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.0.nth(n)
    }
    fn collect<C>(self) -> C
    where
        C: FromIterator<Self::Item>,
    {
        self.0.collect()
    }
}
impl FusedIterator for SectionIter<'_> {}
impl ExactSizeIterator for SectionIter<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }
}
impl DoubleEndedIterator for SectionIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back()
    }
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.0.nth_back(n)
    }
}

/// Iterator over attributes in a section.
pub struct AttrIter<'a>(Iter<'a, AttrKey, Vec<String>>);

impl<'a> Iterator for AttrIter<'a> {
    type Item = (&'a AttrKey, &'a Vec<String>);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
    fn count(self) -> usize {
        self.0.count()
    }
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.0.nth(n)
    }
    fn collect<C>(self) -> C
    where
        C: FromIterator<Self::Item>,
    {
        self.0.collect()
    }
}
impl FusedIterator for AttrIter<'_> {}
impl ExactSizeIterator for AttrIter<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }
}
impl DoubleEndedIterator for AttrIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back()
    }
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.0.nth_back(n)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn key_no_value() {
        let entry = Entry::parse("[Unit]\nName=").unwrap();
        let section = entry.section("Unit").unwrap();
        assert_eq!(section.attr("Name"), &[""]);
        assert!(section.has_attr("Name"));
    }

    #[test]
    fn parse_icon_index() {
        let entry =
            Entry::parse(include_bytes!("./../test_data/gnome-index.theme"))
                .unwrap();
        assert_eq!(entry.sections().len(), 68);
        let status48 = entry.section("48x48/status").unwrap();
        assert_eq!(status48.attr("Size"), &["48"]);
    }

    #[test]
    fn parse_firefox_desktop_entry() {
        let entry =
            Entry::parse(include_bytes!("./../test_data/firefox.desktop"))
                .unwrap();
        assert_eq!(entry.sections().len(), 3);
        let desktop_entry = entry.section("Desktop Entry").unwrap();
        assert_eq!(desktop_entry.attr("Name"), &["Firefox"]);
        assert_eq!(
            desktop_entry.attr_with_param("GenericName", "ast"),
            &["Restolador Web"]
        );
        assert_eq!(
            desktop_entry.attr_with_param("GenericName", "ar"),
            &["متصفح ويب"]
        );
        assert_eq!(
            desktop_entry.attr("Exec"),
            &["/usr/lib/firefox/firefox %u"]
        );
    }

    #[test]
    fn parse_sshd_systemd_unit() {
        let entry = Entry::parse(include_bytes!("./../test_data/sshd.service"))
            .unwrap();
        assert_eq!(entry.sections().len(), 3);
        let section = entry.section("Service").unwrap();
        assert_eq!(section.attr("ExecReload"), &["/bin/kill -HUP $MAINPID"]);
    }

    #[test]
    fn parse_systemd_test() {
        let entry =
            Entry::parse(include_bytes!("./../test_data/edge-cases.txt"))
                .unwrap();
        assert_eq!(entry.sections().len(), 3);
        let section_a = entry.section("Section A").unwrap();
        assert_eq!(section_a.attr("KeyOne"), &["value 1"]);
        assert_eq!(section_a.attr("KeyTwo"), &["value 2"]);

        let section_b = entry.section("Section B").unwrap();
        assert_eq!(
            section_b.attr("Setting"),
            &[r#""something" "some thing" "…""#]
        );
        assert_eq!(section_b.attr("KeyTwo"), &["value 2 value 2 continued"]);

        let section_c = entry.section("Section C").unwrap();
        assert_eq!(section_c.attr("KeyThree"), &["value 3 value 3 continued"]);
    }
}
