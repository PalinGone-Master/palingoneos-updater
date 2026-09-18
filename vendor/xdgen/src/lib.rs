use fluent::{FluentBundle, FluentResource};
use std::{collections::BTreeMap, error::Error, fmt::Write, fs, path::Path};
use unic_langid::LanguageIdentifier;

pub struct Context {
    lang_bundles: BTreeMap<LanguageIdentifier, FluentBundle<FluentResource>>,
}

impl Context {
    pub fn new(
        i18n_dir: impl AsRef<Path>,
        domain: impl AsRef<str>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut lang_files = BTreeMap::new();
        for entry in fs::read_dir(i18n_dir)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_dir() {
                continue;
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|err| format!("invalid UTF-8: {:?}", err))?;
            let lang: LanguageIdentifier = name.parse()?;
            let path = entry
                .path()
                .join(&format!("{}.ftl", domain.as_ref().replace("-", "_")));
            lang_files.insert(lang, path);
        }

        let mut lang_bundles = BTreeMap::new();
        for (lang, path) in lang_files {
            let source = fs::read_to_string(&path)?;
            let res = match FluentResource::try_new(source) {
                Ok(res) => res,
                Err((res, errs)) => {
                    eprintln!(
                        "failed to parse {} with {} errors:",
                        path.display(),
                        errs.len()
                    );
                    for err in errs {
                        eprintln!(" - {}", err);
                    }
                    res
                }
            };
            let mut bundle = FluentBundle::new(vec![lang.clone()]);
            match bundle.add_resource(res) {
                Ok(()) => {}
                Err(errs) => {
                    eprintln!(
                        "failed to add resource {} with {} errors:",
                        path.display(),
                        errs.len()
                    );
                    for err in errs {
                        eprintln!(" - {}", err);
                    }
                }
            }
            lang_bundles.insert(lang, bundle);
        }

        Ok(Self { lang_bundles })
    }
}

#[derive(Clone, Debug)]
pub struct FluentString(pub &'static str);

impl FluentString {
    pub fn get(&self, ctx: &Context) -> BTreeMap<LanguageIdentifier, String> {
        let mut results = BTreeMap::new();
        for (lang, bundle) in ctx.lang_bundles.iter() {
            let Some(msg) = bundle.get_message(self.0) else {
                continue;
            };
            let Some(pat) = msg.value() else { continue };
            let mut errs = Vec::new();
            let result = bundle.format_pattern(&pat, None, &mut errs);
            if !errs.is_empty() {
                eprintln!(
                    "{} errors when formatting {} for lang {}:",
                    errs.len(),
                    self.0,
                    lang
                );
                for err in errs {
                    eprintln!(" - {}", err);
                }
            }
            results.insert(lang.clone(), result.into());
        }
        results
    }
}

#[derive(Clone, Debug)]
pub struct App {
    name: FluentString,
    comment: Option<FluentString>,
    keywords: Option<FluentString>,
}

impl App {
    pub fn new(name: FluentString) -> Self {
        Self {
            name,
            comment: None,
            keywords: None,
        }
    }

    pub fn comment(mut self, value: FluentString) -> Self {
        self.comment = Some(value);
        self
    }

    pub fn keywords(mut self, value: FluentString) -> Self {
        self.keywords = Some(value);
        self
    }

    pub fn expand_desktop(
        &self,
        template_path: impl AsRef<Path>,
        ctx: &Context,
    ) -> Result<String, Box<dyn Error>> {
        let template_path = template_path.as_ref();
        let template = freedesktop_entry_parser::parse_entry(template_path)?;
        let mut s = String::new();
        for (name, section) in template.sections() {
            writeln!(s, "[{}]", name)?;

            for (key, values) in section.attrs() {
                for value in values {
                    write!(s, "{}", key.key)?;
                    if let Some(param) = &key.param {
                        write!(s, "[{}]", param)?;
                    }
                    writeln!(s, "={}", value)?;
                }

                let fluent_opt = match (name.as_str(), key.key.as_str()) {
                    ("Desktop Entry", "Name") => Some(&self.name),
                    ("Desktop Entry", "Comment") => self.comment.as_ref(),
                    ("Desktop Entry", "Keywords") => self.keywords.as_ref(),
                    _ => None,
                };
                if let Some(fluent) = fluent_opt {
                    match &key.param {
                        Some(param) => {
                            return Err(format!(
                                "template {} has localized {}[{}]",
                                template_path.display(),
                                key.key,
                                param,
                            )
                            .into());
                        }
                        None => {
                            // Inject translated names
                            for (lang, value) in fluent.get(ctx) {
                                writeln!(
                                    s,
                                    "{}[{}]={}",
                                    key.key,
                                    lang.to_string().replace("-", "_"),
                                    value
                                )?;
                            }
                        }
                    }
                }
            }
        }

        Ok(s)
    }

    pub fn expand_metainfo(
        &self,
        template_path: impl AsRef<Path>,
        ctx: &Context,
    ) -> Result<String, Box<dyn Error>> {
        use xmltree::{Element, XMLNode};

        let template_path = template_path.as_ref();
        let template = fs::File::open(template_path)?;

        let mut element = Element::parse(template)?;

        let expand_locale = |element: &mut Element,
                             tag: &str,
                             fluent: &FluentString|
         -> Result<(), Box<dyn Error>> {
            let mut index_opt = None;
            for (index, child) in element.children.iter().enumerate() {
                if let Some(element) = child.as_element() {
                    if element.name == tag {
                        if !element.attributes.is_empty() {
                            return Err(format!(
                                "template {} has localized tag {}",
                                template_path.display(),
                                tag
                            )
                            .into());
                        }
                        if index_opt.is_some() {
                            return Err(format!(
                                "template {} has redefined tag {}",
                                template_path.display(),
                                tag
                            )
                            .into());
                        }
                        index_opt = Some(index);
                    }
                }
            }

            let Some(mut index) = index_opt else {
                return Err(format!(
                    "template {} is missing tag {}",
                    template_path.display(),
                    tag
                )
                .into());
            };

            for (lang, value) in fluent.get(ctx) {
                let mut child = Element::new(tag);
                child
                    .attributes
                    .insert("lang".to_string(), lang.to_string().replace("-", "_"));
                child.children.push(XMLNode::Text(value));
                index += 1;
                element.children.insert(index, XMLNode::Element(child));
            }

            Ok(())
        };
        expand_locale(&mut element, "name", &self.name)?;

        if let Some(comment) = &self.comment {
            expand_locale(&mut element, "summary", comment)?;
        }

        if let Some(keywords) = &self.keywords {
            let kw_elem = element.get_mut_child("keywords").ok_or_else(|| {
                format!("template {} is missing keywords", template_path.display())
            })?;
            for (lang, values) in keywords.get(ctx) {
                for value in values.split_terminator(';') {
                    let mut child = Element::new("keyword");
                    child
                        .attributes
                        .insert("lang".to_string(), lang.to_string().replace("-", "_"));
                    child.children.push(XMLNode::Text(value.to_string()));
                    kw_elem.children.push(XMLNode::Element(child));
                }
            }
        }

        let mut data = Vec::new();
        element.write_with_config(
            &mut data,
            xmltree::EmitterConfig::new().perform_indent(true),
        )?;

        let mut s = String::from_utf8(data)?;
        // Hack to re-add xml namespace to lang: https://github.com/eminence/xmltree-rs/issues/13
        s = s.replace(" lang=", " xml:lang=");
        Ok(s)
    }
}
