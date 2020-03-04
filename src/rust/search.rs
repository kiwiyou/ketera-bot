use crate::util::WEB_CLIENT;
use lazy_static::lazy_static;
use reqwest::{header, StatusCode};
use scraper::{ElementRef, Html, Selector};
use selectors::attr::CaseSensitivity;

struct CrateStructure<'a> {
    module: &'a [&'a str],
    name: &'a str,
    structure_type: StructureType,
}

enum StructureType {
    Module,
    Function,
    Struct,
    Trait,
    Method,
    TraitMethod,
}

pub struct CrateDocument {
    /// Title of this document.
    /// e.g. Struct ketera_bot::rust::search::CrateDocument
    pub title: String,
    /// Definition of the content.
    pub definition: Option<String>,
    /// Portability of the content.
    /// e.g. feature="derive"
    pub portability_note: Option<String>,
    /// Sability of the content.
    /// e.g. Experimental (never_type #35121)
    pub stability_note: Option<String>,
    pub deprecated: bool,
    /// Description of the content.
    /// It is a combined text of all paragraphs before the first heading.
    pub description: String,
    /// Additional sections of this document.
    /// It consists of pairs of the heading and the article.
    pub sections: Vec<(String, Article)>,
}

#[derive(Debug)]
pub enum Article {
    Text(String),
    SubDocuments(Vec<SubDocument>),
}

#[derive(Debug)]
pub struct SubDocument {
    pub name: String,
    /// Portability of the content.
    /// e.g. feature="derive"
    pub portability_note: Option<String>,
    /// Stability of the content.
    /// e.g. Experimental (never_type #35121)
    pub stability_note: Option<String>,
    pub deprecated: bool,
    /// Description (in module index) if exists.
    pub summary: Option<String>,
}

impl<'a> CrateStructure<'a> {
    async fn get_document(&self, crate_location: &str) -> reqwest::Result<Option<CrateDocument>> {
        lazy_static! {
            static ref TITLE_SELECTOR: Selector = Selector::parse(".fqn > .in-band").unwrap();
            static ref PORTABILITY_SELECTOR: Selector =
                Selector::parse("#main > .stability > .portability").unwrap();
            static ref STABILITY_SELECTOR: Selector =
                Selector::parse("#main > .stability > .unstable").unwrap();
            static ref DEPRECATION_SELECTOR: Selector =
                Selector::parse("#main > .stability > .deprecated").unwrap();
            static ref MODULES_SELECTOR: Selector = Selector::parse("#modules + table tr").unwrap();
            static ref STRUCTS_SELECTOR: Selector = Selector::parse("#structs + table tr").unwrap();
            static ref TRAITS_SELECTOR: Selector = Selector::parse("#traits + table tr").unwrap();
            static ref ENUMS_SELECTOR: Selector = Selector::parse("#enums + table tr").unwrap();
            static ref MACROS_SELECTOR: Selector = Selector::parse("#macros + table tr").unwrap();
            static ref FUNCTIONS_SELECTOR: Selector =
                Selector::parse("#functions + table tr").unwrap();
            static ref ATTRIBUTES_SELECTOR: Selector =
                Selector::parse("#attributes + table tr").unwrap();
            static ref CONSTS_SELECTOR: Selector = Selector::parse("#consts + table tr").unwrap();
            static ref DEFINITION_SELECTOR: Selector =
                Selector::parse("#main > .type_decl > pre").unwrap();
            static ref DOCBLOCK_SELECTOR: Selector =
                Selector::parse("#main > div.docblock:not(.type-decl)").unwrap();
            static ref METHODS_SELECTOR: Selector =
                Selector::parse("#impl + .impl-items h4 > code").unwrap();
            static ref IMPLS_SELECTOR: Selector =
                Selector::parse("#implementations-list .in-band").unwrap();
            static ref REQUIRED_METHODS_SELECTOR: Selector =
                Selector::parse("#required-methods + .methods .method > code").unwrap();
            static ref PROVIDED_METHODS_SELECTOR: Selector =
                Selector::parse("#provided-methods + .methods .method > code").unwrap();
            static ref TRAIT_IMPLS_SELECTOR: Selector =
                Selector::parse("#main > .impl .in-band").unwrap();
            static ref TRAIT_IMPLORS_SELECTOR: Selector =
                Selector::parse("#implementors-list .in-band").unwrap();
            static ref METHOD_DEFINITION_SELECTOR: Selector = Selector::parse("code").unwrap();
            static ref METHOD_PORTABILITY_SELECTOR: Selector =
                Selector::parse(".portability").unwrap();
            static ref METHOD_STABILITY_SELECTOR: Selector = Selector::parse(".unstable").unwrap();
            static ref METHOD_DEPRECATION_SELECTOR: Selector =
                Selector::parse(".deprecated").unwrap();
        }

        let html = match self.get_html(crate_location).await? {
            Some(html) => html,
            None => return Ok(None),
        };

        let title = html.select(&TITLE_SELECTOR).next().unwrap();
        let (definition, portability_note, stability_note, deprecated, docblock) =
            match self.structure_type {
                StructureType::Method | StructureType::TraitMethod => {
                    let mut portability = None;
                    let mut stability = None;
                    let mut deprecated = false;
                    let mut docblock = None;

                    let selector_text = format!(
                        "#tymethod\\.{method}, #method\\.{method}",
                        method = self.name
                    );
                    let selector = match Selector::parse(&selector_text) {
                        Ok(selector) => selector,
                        Err(_) => return Ok(None),
                    };
                    let definition_wrapper = html.select(&selector).next().unwrap();
                    let definition = definition_wrapper
                        .select(&METHOD_DEFINITION_SELECTOR)
                        .next()
                        .map(code_node_text);
                    for sibling in definition_wrapper
                        .next_siblings()
                        .filter_map(ElementRef::wrap)
                    {
                        let element = sibling.value();
                        if element.name() != "div" {
                            break;
                        }
                        if element.has_class("stability", CaseSensitivity::CaseSensitive) {
                            portability = sibling
                                .select(&METHOD_PORTABILITY_SELECTOR)
                                .next()
                                .map(node_text);
                            stability = sibling
                                .select(&METHOD_STABILITY_SELECTOR)
                                .next()
                                .map(node_text);
                            deprecated = sibling
                                .select(&METHOD_DEPRECATION_SELECTOR)
                                .next()
                                .is_some();
                        } else if element.has_class("docblock", CaseSensitivity::CaseSensitive) {
                            docblock = Some(sibling);
                        }
                    }
                    // Docblock must be present
                    (
                        definition,
                        portability,
                        stability,
                        deprecated,
                        docblock.unwrap(),
                    )
                }
                _ => {
                    let definition = html.select(&DEFINITION_SELECTOR).next().map(code_node_text);
                    let portability = html.select(&PORTABILITY_SELECTOR).next().map(node_text);
                    let stability = html.select(&STABILITY_SELECTOR).next().map(node_text);
                    let deprecated = html.select(&DEPRECATION_SELECTOR).next().is_some();
                    let docblock = html.select(&DOCBLOCK_SELECTOR).next().unwrap();
                    (definition, portability, stability, deprecated, docblock)
                }
            };

        let mut sections = Vec::new();
        let mut buffer = Vec::new();
        for doc_element in docblock.children().filter_map(ElementRef::wrap).rev() {
            if doc_element.value().name() == "h1" {
                buffer.reverse();
                sections.push((node_text(doc_element), Article::Text(buffer.join("\n"))));
                buffer.clear();
            } else if let Some(paragraph) = parse_document_paragraph(doc_element) {
                buffer.push(paragraph);
            }
        }
        buffer.reverse();
        let description = buffer.join("\n");

        macro_rules! add_subdocuments {
            ($name:literal, $selector:ident) => {
                let subdocuments: Vec<SubDocument> =
                    html.select(&$selector).map(parse_subdocument).collect();
                if !subdocuments.is_empty() {
                    sections.push(($name.into(), Article::SubDocuments(subdocuments)));
                }
            };
        }

        match self.structure_type {
            StructureType::Module => {
                macro_rules! add_module_subdocuments {
                    ($name:literal, $selector:ident) => {
                        let subdocuments: Vec<SubDocument> = html
                            .select(&$selector)
                            .map(parse_module_subdocument)
                            .collect();
                        if !subdocuments.is_empty() {
                            sections.push(($name.into(), Article::SubDocuments(subdocuments)));
                        }
                    };
                }
                add_module_subdocuments!("Modules", MODULES_SELECTOR);
                add_module_subdocuments!("Structs", STRUCTS_SELECTOR);
                add_module_subdocuments!("Traits", TRAITS_SELECTOR);
                add_module_subdocuments!("Enums", ENUMS_SELECTOR);
                add_module_subdocuments!("Macros", MACROS_SELECTOR);
                add_module_subdocuments!("Functions", FUNCTIONS_SELECTOR);
                add_module_subdocuments!("Attributes", ATTRIBUTES_SELECTOR);
                add_module_subdocuments!("Constants", CONSTS_SELECTOR);
            }
            StructureType::Struct => {
                add_subdocuments!("Methods", METHODS_SELECTOR);
                add_subdocuments!("Trait Implementations", IMPLS_SELECTOR);
            }
            StructureType::Trait => {
                add_subdocuments!("Required Methods", REQUIRED_METHODS_SELECTOR);
                add_subdocuments!("Provided Methods", PROVIDED_METHODS_SELECTOR);
                add_subdocuments!("Foreign Implementations", TRAIT_IMPLS_SELECTOR);
                add_subdocuments!("Implementors", TRAIT_IMPLORS_SELECTOR);
            }
            _ => {}
        }
        Ok(Some(CrateDocument {
            title: node_text(title),
            definition,
            deprecated,
            description,
            portability_note,
            sections,
            stability_note,
        }))
    }

    async fn get_html(&self, crate_location: &str) -> reqwest::Result<Option<Html>> {
        let mut url = crate_location.to_string();
        // without crate name
        let effective_module = &self.module[1..];
        match self.structure_type {
            StructureType::Module => {
                // module/submodule/index.html
                url.push_str(&effective_module.join("/"));
                if !effective_module.is_empty() {
                    url.push('/');
                }
                url.push_str("index.html");
            }
            StructureType::Function => {
                // module/submodule/fn.foo.html
                url.push_str(&effective_module.join("/"));
                if !effective_module.is_empty() {
                    url.push('/');
                }
                url.push_str("fn.");
                url.push_str(self.name);
                url.push_str(".html");
            }
            StructureType::Struct => {
                // module/submodule/struct.foo.html
                url.push_str(&effective_module.join("/"));
                if !effective_module.is_empty() {
                    url.push('/');
                }
                url.push_str("struct");
                url.push_str(self.name);
                url.push_str(".html");
            }
            StructureType::Trait => {
                // module/submodule/trait.foo.html
                url.push_str(&effective_module.join("/"));
                if !effective_module.is_empty() {
                    url.push('/');
                }
                url.push_str("/trait");
                url.push_str(self.name);
                url.push_str(".html");
            }
            StructureType::Method => {
                // module/submodule/struct.foo.html#method.bar
                let effective_module = &effective_module[..effective_module.len() - 1];
                url.push_str(&effective_module.join("/"));
                if !effective_module.is_empty() {
                    url.push('/');
                }
                url.push_str("struct");
                url.push_str(self.module[self.module.len() - 1]);
                url.push_str(".html");
            }
            StructureType::TraitMethod => {
                // module/submodule/trait.foo.html#tymethod.bar OR
                // module/submodule/trait.foo.html#method.bar
                let effective_module = &effective_module[..effective_module.len() - 1];
                url.push_str(&effective_module.join("/"));
                if !effective_module.is_empty() {
                    url.push('/');
                }
                url.push_str("trait");
                url.push_str(self.module[self.module.len() - 1]);
                url.push_str(".html");
            }
        }

        let response = WEB_CLIENT.get(&url).send().await?;
        if !response.status().is_success() {
            Ok(None)
        } else {
            Ok(Some(Html::parse_document(&response.text().await?)))
        }
    }
}

pub async fn get_document(path: &str) -> reqwest::Result<Option<CrateDocument>> {
    use tokio::try_join;
    let tree: Vec<_> = path.split("::").collect();
    if tree.is_empty() {
        return Ok(None);
    }
    let c = if let Some(c) = get_latest_document(tree[0]).await? {
        c
    } else {
        return Ok(None);
    };
    let tree = &tree[..];
    let result = if tree.len() == 1 {
        CrateStructure {
            module: tree,
            name: tree[0],
            structure_type: StructureType::Module,
        }
        .get_document(&c)
        .await?
    } else if tree.len() == 2 {
        let module_candidate = CrateStructure {
            module: tree,
            name: tree[1],
            structure_type: StructureType::Module,
        };
        let function_candidate = CrateStructure {
            module: &tree[..1],
            name: tree[1],
            structure_type: StructureType::Function,
        };
        let struct_candidate = CrateStructure {
            structure_type: StructureType::Struct,
            ..function_candidate
        };
        let trait_candidate = CrateStructure {
            structure_type: StructureType::Struct,
            ..function_candidate
        };
        let (maybe_module, maybe_function, maybe_struct, maybe_trait) = try_join!(
            module_candidate.get_document(&c),
            function_candidate.get_document(&c),
            struct_candidate.get_document(&c),
            trait_candidate.get_document(&c)
        )?;
        match (maybe_module, maybe_function, maybe_struct, maybe_trait) {
            (Some(found), _, _, _)
            | (_, Some(found), _, _)
            | (_, _, Some(found), _)
            | (_, _, _, Some(found)) => Some(found),
            _ => None,
        }
    } else {
        let module_candidate = CrateStructure {
            module: tree,
            name: tree[tree.len() - 1],
            structure_type: StructureType::Module,
        };
        let function_candidate = CrateStructure {
            module: &tree[..tree.len() - 1],
            name: tree[tree.len() - 1],
            structure_type: StructureType::Function,
        };
        let struct_candidate = CrateStructure {
            structure_type: StructureType::Struct,
            ..function_candidate
        };
        let trait_candidate = CrateStructure {
            structure_type: StructureType::Trait,
            ..function_candidate
        };
        let method_candidate = CrateStructure {
            structure_type: StructureType::Method,
            ..function_candidate
        };
        let trait_method_candidate = CrateStructure {
            structure_type: StructureType::TraitMethod,
            ..function_candidate
        };
        let (
            maybe_module,
            maybe_function,
            maybe_struct,
            maybe_trait,
            maybe_method,
            maybe_trait_method,
        ) = try_join!(
            module_candidate.get_document(&c),
            function_candidate.get_document(&c),
            struct_candidate.get_document(&c),
            trait_candidate.get_document(&c),
            method_candidate.get_document(&c),
            trait_method_candidate.get_document(&c)
        )?;
        match (
            maybe_module,
            maybe_function,
            maybe_struct,
            maybe_trait,
            maybe_method,
            maybe_trait_method,
        ) {
            (Some(found), _, _, _, _, _)
            | (_, Some(found), _, _, _, _)
            | (_, _, Some(found), _, _, _)
            | (_, _, _, Some(found), _, _)
            | (_, _, _, _, Some(found), _)
            | (_, _, _, _, _, Some(found)) => Some(found),
            _ => None,
        }
    };
    Ok(result)
}

fn node_text(item: ElementRef) -> String {
    item.text().collect()
}

fn code_node_text(code: ElementRef) -> String {
    let concatted = code.text().fold(String::new(), |mut acc: String, s: &str| {
        if s == "where" || s.starts_with(char::is_whitespace) {
            acc.push('\n');
        }
        acc.push_str(s);
        acc
    });
    format!(
        "<pre><code class=\"language-rust\">{}</code></pre>",
        crate::util::escape_html_entities(&concatted)
    )
}

fn parse_module_subdocument(item: ElementRef) -> SubDocument {
    lazy_static! {
        static ref NAME_SELECTOR: Selector = Selector::parse("td").unwrap();
        static ref DEPRECATED_SELECTOR: Selector = Selector::parse(".deprecated").unwrap();
        static ref PORTABILITY_SELECTOR: Selector = Selector::parse(".portability").unwrap();
        static ref STABILITY_SELECTOR: Selector = Selector::parse(".unstable").unwrap();
        static ref SUMMARY_SELECTOR: Selector = Selector::parse(".docblock-short > p").unwrap();
    }

    let name = node_text(item.select(&NAME_SELECTOR).next().unwrap());
    let is_deprecated = item.select(&DEPRECATED_SELECTOR).next().is_some();
    let portability = item.select(&PORTABILITY_SELECTOR).next().map(node_text);
    let stability = item.select(&STABILITY_SELECTOR).next().map(node_text);
    let summary = item.select(&SUMMARY_SELECTOR).next().unwrap().inner_html();
    SubDocument {
        name,
        portability_note: portability,
        stability_note: stability,
        deprecated: is_deprecated,
        summary: Some(summary),
    }
}

fn parse_subdocument(item: ElementRef) -> SubDocument {
    lazy_static! {
        static ref DEPRECATED_SELECTOR: Selector = Selector::parse(".deprecated").unwrap();
        static ref PORTABILITY_SELECTOR: Selector = Selector::parse(".portability").unwrap();
        static ref STABILITY_SELECTOR: Selector = Selector::parse(".unstable").unwrap();
    }

    let name = code_node_text(item);
    let deprecated = item.select(&DEPRECATED_SELECTOR).next().is_some();
    let portability = item.select(&PORTABILITY_SELECTOR).next().map(node_text);
    let stability = item.select(&STABILITY_SELECTOR).next().map(node_text);

    SubDocument {
        name,
        portability_note: portability,
        stability_note: stability,
        deprecated,
        summary: None,
    }
}

fn parse_document_paragraph(paragraph: ElementRef) -> Option<String> {
    use regex::Regex;
    lazy_static::lazy_static! {
        static ref DANGLING_LINK: Regex = Regex::new(r#"<a href="[^h].*">([\s\S]*)</a>"#).unwrap();
    }
    match paragraph.value().name() {
        "p" => {
            let inner_html = paragraph.inner_html();
            let dangling_link_removed = DANGLING_LINK.replace_all(&inner_html, "$1");
            Some(dangling_link_removed.to_string())
        }
        "div" => Some(format!(
            "<pre><code class=\"language-rust\">{}</code></pre>",
            crate::util::escape_html_entities(&node_text(paragraph))
        )),
        _ => None,
    }
}

// returns the root url of document without a slash
async fn get_latest_document(crate_name: &str) -> reqwest::Result<Option<String>> {
    if let Some(std) = get_std_rs(crate_name) {
        Ok(Some(std))
    } else {
        get_docs_rs(crate_name).await
    }
}

fn get_std_rs(crate_name: &str) -> Option<String> {
    match crate_name {
        "alloc" | "core" | "proc_macro" | "std" | "text" => {
            Some(format!("https://doc.rust-lang.org/stable/{}/", crate_name))
        }
        _ => None,
    }
}

async fn get_docs_rs(crate_name: &str) -> reqwest::Result<Option<String>> {
    let response = WEB_CLIENT
        .get(&format!("https://docs.rs/{}", crate_name))
        .send()
        .await?;
    if response.status() == StatusCode::FOUND {
        let location = response
            .headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap();
        let mut location = location.to_owned();
        if location.chars().rev().next() != Some('/') {
            location.push('/');
        }
        Ok(Some(location))
    } else {
        Ok(None)
    }
}
