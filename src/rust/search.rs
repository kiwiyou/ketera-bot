use crate::util::WEB_CLIENT;
use lazy_static::lazy_static;
use reqwest::{header, StatusCode};
use scraper::{ElementRef, Html, Selector};
use selectors::attr::CaseSensitivity;

enum CrateStructure<'a> {
    Module {
        module: &'a [&'a str],
    },
    Function {
        module: &'a [&'a str],
        name: &'a str,
    },
    Struct {
        module: &'a [&'a str],
        name: &'a str,
    },
    Trait {
        module: &'a [&'a str],
        name: &'a str,
    },
    Method {
        module: &'a [&'a str],
        r#struct: &'a str,
        name: &'a str,
    },
    TraitMethod {
        module: &'a [&'a str],
        r#trait: &'a str,
        name: &'a str,
    },
}

impl<'a> CrateStructure<'a> {
    async fn get_document(&self, document: &Crate) -> reqwest::Result<Option<CrateDocument>> {
        lazy_static! {
            static ref PORTABILITY_SELECTOR: Selector =
                Selector::parse("#main > .stability strong").unwrap();
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
            static ref DEFINITION_SELECTOR: Selector = Selector::parse("pre").unwrap();
            static ref DOCBLOCK_SELECTOR: Selector =
                Selector::parse("div.docblock:not(.type-decl").unwrap();
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
            static ref METHOD_PORTABILITY_SELECTOR: Selector = Selector::parse("strong").unwrap();
        }

        let result = match self {
            Self::Module { module } => {
                let tree = module[1..].iter().fold(String::new(), |mut s, c| {
                    s.push_str(&c);
                    s.push('/');
                    s
                });
                let url = format!(
                    "{prefix}{tree}index.html",
                    prefix = document.url,
                    tree = tree
                );
                let response = WEB_CLIENT.get(&url).send().await?;
                if !response.status().is_success() {
                    return Ok(None);
                }
                let html = Html::parse_document(response.text().await?.as_ref());
                let portability = html.select(&PORTABILITY_SELECTOR).next().map(node_text);
                CrateDocument::Module {
                    path: module.join("::"),
                    portability,
                    modules: html
                        .select(&MODULES_SELECTOR)
                        .map(parse_document_brief)
                        .collect(),
                    structs: html
                        .select(&STRUCTS_SELECTOR)
                        .map(parse_document_brief)
                        .collect(),
                    traits: html
                        .select(&TRAITS_SELECTOR)
                        .map(parse_document_brief)
                        .collect(),
                    enums: html
                        .select(&ENUMS_SELECTOR)
                        .map(parse_document_brief)
                        .collect(),
                    macros: html
                        .select(&MACROS_SELECTOR)
                        .map(parse_document_brief)
                        .collect(),
                    functions: html
                        .select(&FUNCTIONS_SELECTOR)
                        .map(parse_document_brief)
                        .collect(),
                    attributes: html
                        .select(&ATTRIBUTES_SELECTOR)
                        .map(parse_document_brief)
                        .collect(),
                    consts: html
                        .select(&CONSTS_SELECTOR)
                        .map(parse_document_brief)
                        .collect(),
                }
            }
            Self::Function { module, name } => {
                let tree = module[1..].iter().fold(String::new(), |mut s, c| {
                    s.push_str(&c);
                    s.push('/');
                    s
                });
                let url = format!(
                    "{prefix}{tree}fn.{name}.html",
                    prefix = document.url,
                    tree = tree,
                    name = name
                );
                let response = WEB_CLIENT.get(&url).send().await?;
                if !response.status().is_success() {
                    return Ok(None);
                }
                let html = Html::parse_document(&response.text().await?);
                let definition = html.select(&DEFINITION_SELECTOR).next().unwrap();
                let portability = html.select(&PORTABILITY_SELECTOR).next().map(node_text);
                let inner = html.select(&DOCBLOCK_SELECTOR).next().unwrap();
                let mut sections = Vec::new();
                let mut buffer = Vec::new();
                for paragraph in inner.children().filter_map(ElementRef::wrap).rev() {
                    if paragraph.value().name() == "h1" {
                        buffer.reverse();
                        sections.push((node_text(paragraph), buffer.join("\n")));
                        buffer.clear();
                    } else if let Some(text) = parse_document_paragraph(paragraph) {
                        buffer.push(text);
                    }
                }
                buffer.reverse();
                let description = buffer.join("\n");
                CrateDocument::Function {
                    path: format!("{}::{}", module.join("::"), name),
                    definition: code_node_text(definition),
                    portability,
                    description,
                    sections,
                }
            }
            Self::Struct { module, name } => {
                let tree = module[1..].iter().fold(String::new(), |mut s, c| {
                    s.push_str(&c);
                    s.push('/');
                    s
                });
                let url = format!(
                    "{prefix}{tree}struct.{name}.html",
                    prefix = document.url,
                    tree = tree,
                    name = name
                );
                let response = WEB_CLIENT.get(&url).send().await?;
                if !response.status().is_success() {
                    return Ok(None);
                }
                let html = Html::parse_document(&response.text().await?);
                let definition = html.select(&DEFINITION_SELECTOR).next().unwrap();
                let portability = html.select(&PORTABILITY_SELECTOR).next().map(node_text);
                let inner = html.select(&DOCBLOCK_SELECTOR).next().unwrap();
                let mut sections = Vec::new();
                let mut buffer = Vec::new();
                for paragraph in inner.children().filter_map(ElementRef::wrap).rev() {
                    if paragraph.value().name() == "h1" {
                        buffer.reverse();
                        sections.push((node_text(paragraph), buffer.join("\n")));
                        buffer.clear();
                    } else if let Some(text) = parse_document_paragraph(paragraph) {
                        buffer.push(text);
                    }
                }
                buffer.reverse();
                let description = buffer.join("\n");
                let methods = html.select(&IMPLS_SELECTOR).map(code_node_text).collect();
                let implementations = html.select(&IMPLS_SELECTOR).map(code_node_text).collect();
                CrateDocument::Struct {
                    path: format!("{}::{}", module.join("::"), name),
                    definition: code_node_text(definition),
                    portability,
                    description,
                    sections,
                    methods,
                    implementations,
                }
            }
            Self::Trait { module, name } => {
                let tree = module[1..].iter().fold(String::new(), |mut s, c| {
                    s.push_str(&c);
                    s.push('/');
                    s
                });
                let url = format!(
                    "{prefix}{tree}trait.{name}.html",
                    prefix = document.url,
                    tree = tree,
                    name = name
                );
                let response = WEB_CLIENT.get(&url).send().await?;
                if !response.status().is_success() {
                    return Ok(None);
                }
                let html = Html::parse_document(&response.text().await?);
                let definition = html.select(&DEFINITION_SELECTOR).next().unwrap();
                let portability = html.select(&PORTABILITY_SELECTOR).next().map(node_text);
                let inner = html.select(&DOCBLOCK_SELECTOR).next().unwrap();
                let mut sections = Vec::new();
                let mut buffer = Vec::new();
                for paragraph in inner.children().filter_map(ElementRef::wrap).rev() {
                    if paragraph.value().name() == "h1" {
                        buffer.reverse();
                        sections.push((node_text(paragraph), buffer.join("\n")));
                        buffer.clear();
                    } else if let Some(text) = parse_document_paragraph(paragraph) {
                        buffer.push(text);
                    }
                }
                buffer.reverse();
                let description = buffer.join("\n");
                let required_methods = html
                    .select(&REQUIRED_METHODS_SELECTOR)
                    .map(code_node_text)
                    .collect();
                let provided_methods = html
                    .select(&PROVIDED_METHODS_SELECTOR)
                    .map(code_node_text)
                    .collect();
                let implementations = html
                    .select(&TRAIT_IMPLS_SELECTOR)
                    .map(code_node_text)
                    .collect();
                let implementors = html
                    .select(&TRAIT_IMPLORS_SELECTOR)
                    .map(code_node_text)
                    .collect();
                CrateDocument::Trait {
                    path: format!("{}::{}", module.join("::"), name),
                    definition: code_node_text(definition),
                    portability,
                    description,
                    sections,
                    required_methods,
                    provided_methods,
                    implementations,
                    implementors,
                }
            }
            Self::Method {
                module,
                r#struct,
                name,
            } => {
                let tree = module[1..].iter().fold(String::new(), |mut s, c| {
                    s.push_str(&c);
                    s.push('/');
                    s
                });
                let url = format!(
                    "{prefix}{tree}struct.{name}.html",
                    prefix = document.url,
                    tree = tree,
                    name = r#struct
                );
                let response = WEB_CLIENT.get(&url).send().await?;
                if !response.status().is_success() {
                    return Ok(None);
                }
                let html = Html::parse_document(&response.text().await?);
                let function = if let Some(function) = html
                    .select(&Selector::parse(format!("#method\\.{}", name).as_ref()).unwrap())
                    .next()
                {
                    function
                } else {
                    return Ok(None);
                };
                let definition = function.select(&METHOD_DEFINITION_SELECTOR).next().unwrap();
                let mut portability = None;
                let mut next = function.next_sibling();
                match next.and_then(ElementRef::wrap) {
                    Some(stability)
                        if stability
                            .value()
                            .has_class("stability", CaseSensitivity::CaseSensitive) =>
                    {
                        portability = stability
                            .select(&METHOD_PORTABILITY_SELECTOR)
                            .next()
                            .map(node_text);
                        next = stability.next_sibling();
                    }
                    _ => {}
                }
                let mut sections = Vec::new();
                let mut description = String::new();
                match next.and_then(ElementRef::wrap) {
                    Some(docblock)
                        if docblock
                            .value()
                            .has_class("docblock", CaseSensitivity::CaseSensitive) =>
                    {
                        let mut buffer = Vec::new();
                        for paragraph in docblock.children().filter_map(ElementRef::wrap).rev() {
                            if paragraph.value().name() == "h1" {
                                buffer.reverse();
                                sections.push((node_text(paragraph), buffer.join("\n")));
                                buffer.clear();
                            } else if let Some(text) = parse_document_paragraph(paragraph) {
                                buffer.push(text);
                            }
                        }
                        buffer.reverse();
                        description = buffer.join("\n");
                    }
                    _ => {}
                }
                CrateDocument::Method {
                    path: format!("{}::{}::{}", module.join("::"), r#struct, name),
                    definition: code_node_text(definition),
                    portability,
                    description,
                    sections,
                }
            }
            Self::TraitMethod {
                module,
                r#trait,
                name,
            } => {
                let tree = module[1..].iter().fold(String::new(), |mut s, c| {
                    s.push_str(&c);
                    s.push('/');
                    s
                });
                let url = format!(
                    "{prefix}{tree}trait.{name}.html",
                    prefix = document.url,
                    tree = tree,
                    name = r#trait
                );
                let response = WEB_CLIENT.get(&url).send().await?;
                if !response.status().is_success() {
                    return Ok(None);
                }
                let html = Html::parse_document(&response.text().await?);
                let function = if let Some(function) = html
                    .select(
                        &Selector::parse(
                            format!("#tymethod\\.{name}, #method\\.{name}", name = name).as_ref(),
                        )
                        .unwrap(),
                    )
                    .next()
                {
                    function
                } else {
                    return Ok(None);
                };
                let definition = function.select(&METHOD_DEFINITION_SELECTOR).next().unwrap();
                let mut portability = None;
                let mut next = function.next_sibling();
                match next.and_then(ElementRef::wrap) {
                    Some(stability)
                        if stability
                            .value()
                            .has_class("stability", CaseSensitivity::CaseSensitive) =>
                    {
                        portability = stability
                            .select(&METHOD_PORTABILITY_SELECTOR)
                            .next()
                            .map(node_text);
                        next = stability.next_sibling();
                    }
                    _ => {}
                }
                let mut sections = Vec::new();
                let mut description = String::new();
                match next.and_then(ElementRef::wrap) {
                    Some(docblock)
                        if docblock
                            .value()
                            .has_class("docblock", CaseSensitivity::CaseSensitive) =>
                    {
                        let mut buffer = Vec::new();
                        for paragraph in docblock.children().filter_map(ElementRef::wrap).rev() {
                            if paragraph.value().name() == "h1" {
                                buffer.reverse();
                                sections.push((node_text(paragraph), buffer.join("\n")));
                                buffer.clear();
                            } else if let Some(text) = parse_document_paragraph(paragraph) {
                                buffer.push(text);
                            }
                        }
                        buffer.reverse();
                        description = buffer.join("\n");
                    }
                    _ => {}
                }
                CrateDocument::TraitMethod {
                    path: format!("{}::{}::{}", module.join("::"), r#trait, name),
                    definition: code_node_text(definition),
                    portability,
                    description,
                    sections,
                }
            }
        };
        Ok(Some(result))
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
        CrateStructure::Module { module: tree }
            .get_document(&c)
            .await?
    } else if tree.len() == 2 {
        let module_candidate = CrateStructure::Module { module: tree };
        let function_candidate = CrateStructure::Function {
            module: &tree[..1],
            name: tree[1],
        };
        let struct_candidate = CrateStructure::Struct {
            module: &tree[..1],
            name: tree[1],
        };
        let trait_candidate = CrateStructure::Trait {
            module: &tree[..1],
            name: tree[1],
        };
        let (m, f, s, t) = try_join!(
            module_candidate.get_document(&c),
            function_candidate.get_document(&c),
            struct_candidate.get_document(&c),
            trait_candidate.get_document(&c)
        )?;
        match (m, f, s, t) {
            (Some(r), _, _, _) | (_, Some(r), _, _) | (_, _, Some(r), _) | (_, _, _, Some(r)) => {
                Some(r)
            }
            _ => None,
        }
    } else {
        let module_candidate = CrateStructure::Module { module: tree };
        let function_candidate = CrateStructure::Function {
            module: &tree[..tree.len() - 1],
            name: tree[tree.len() - 1],
        };
        let struct_candidate = CrateStructure::Struct {
            module: &tree[..tree.len() - 1],
            name: tree[tree.len() - 1],
        };
        let trait_candidate = CrateStructure::Trait {
            module: &tree[..tree.len() - 1],
            name: tree[tree.len() - 1],
        };
        let method_candidate = CrateStructure::Method {
            module: &tree[..tree.len() - 2],
            r#struct: tree[tree.len() - 2],
            name: tree[tree.len() - 1],
        };
        let tmethod_candidate = CrateStructure::TraitMethod {
            module: &tree[..tree.len() - 2],
            r#trait: tree[tree.len() - 2],
            name: tree[tree.len() - 1],
        };
        let (m, f, s, t, sm, tm) = try_join!(
            module_candidate.get_document(&c),
            function_candidate.get_document(&c),
            struct_candidate.get_document(&c),
            trait_candidate.get_document(&c),
            method_candidate.get_document(&c),
            tmethod_candidate.get_document(&c)
        )?;
        match (m, f, s, t, sm, tm) {
            (Some(r), _, _, _, _, _)
            | (_, Some(r), _, _, _, _)
            | (_, _, Some(r), _, _, _)
            | (_, _, _, Some(r), _, _)
            | (_, _, _, _, Some(r), _)
            | (_, _, _, _, _, Some(r)) => Some(r),
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

fn parse_document_brief(item: ElementRef) -> DocumentBriefItem {
    lazy_static! {
        static ref NAME_SELECTOR: Selector = Selector::parse("td").unwrap();
        static ref DEPRECATED_SELECTOR: Selector = Selector::parse(".deprecated").unwrap();
        static ref PORTABILITY_SELECTOR: Selector = Selector::parse(".portability").unwrap();
        static ref DESCRIPTION_SELECTOR: Selector = Selector::parse(".docblock-short > p").unwrap();
    }

    let name = node_text(item.select(&NAME_SELECTOR).next().unwrap());
    let is_deprecated = item.select(&DEPRECATED_SELECTOR).next().is_some();
    let portability = item.select(&PORTABILITY_SELECTOR).next().map(node_text);
    let description = item
        .select(&DESCRIPTION_SELECTOR)
        .next()
        .unwrap()
        .inner_html();
    DocumentBriefItem {
        name,
        deprecated: is_deprecated,
        portability,
        description,
    }
}

fn parse_document_paragraph(paragraph: ElementRef) -> Option<String> {
    use regex::Regex;
    lazy_static::lazy_static! {
        static ref DANGLING_LINK: Regex = Regex::new(r#"<a href="[^h].*">([\s\S]*)</a>"#).unwrap();
    }
    match paragraph.value().name() {
        "p" => Some(
            DANGLING_LINK
                .replace_all(&paragraph.inner_html(), "$1")
                .to_string(),
        ),
        "div" => Some(format!(
            "<pre><code class=\"language-rust\">{}</code></pre>",
            crate::util::escape_html_entities(&node_text(paragraph))
        )),
        _ => None,
    }
}

#[derive(Debug)]
pub enum CrateDocument {
    Module {
        path: String,
        portability: Option<String>,
        modules: Vec<DocumentBriefItem>,
        structs: Vec<DocumentBriefItem>,
        traits: Vec<DocumentBriefItem>,
        enums: Vec<DocumentBriefItem>,
        macros: Vec<DocumentBriefItem>,
        functions: Vec<DocumentBriefItem>,
        attributes: Vec<DocumentBriefItem>,
        consts: Vec<DocumentBriefItem>,
    },
    Function {
        path: String,
        definition: String,
        portability: Option<String>,
        description: String,
        sections: Vec<(String, String)>,
    },
    Struct {
        path: String,
        definition: String,
        portability: Option<String>,
        description: String,
        sections: Vec<(String, String)>,
        methods: Vec<String>,
        implementations: Vec<String>,
    },
    Trait {
        path: String,
        definition: String,
        portability: Option<String>,
        description: String,
        sections: Vec<(String, String)>,
        required_methods: Vec<String>,
        provided_methods: Vec<String>,
        implementations: Vec<String>,
        implementors: Vec<String>,
    },
    Method {
        path: String,
        definition: String,
        portability: Option<String>,
        description: String,
        sections: Vec<(String, String)>,
    },
    TraitMethod {
        path: String,
        definition: String,
        portability: Option<String>,
        description: String,
        sections: Vec<(String, String)>,
    },
}

#[derive(Debug)]
pub struct DocumentBriefItem {
    pub name: String,
    pub deprecated: bool,
    pub portability: Option<String>,
    pub description: String,
}

struct Crate {
    url: String,
}

// returns the root url of document without a slash
async fn get_latest_document(crate_name: &str) -> reqwest::Result<Option<Crate>> {
    if let Some(std) = get_std_rs(crate_name) {
        Ok(Some(std))
    } else {
        get_docs_rs(crate_name).await
    }
}

fn get_std_rs(crate_name: &str) -> Option<Crate> {
    match crate_name {
        "alloc" | "core" | "proc_macro" | "std" | "text" => Some(Crate {
            url: format!("https://doc.rust-lang.org/stable/{}/", crate_name),
        }),
        _ => None,
    }
}

async fn get_docs_rs(crate_name: &str) -> reqwest::Result<Option<Crate>> {
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
        Ok(Some(Crate { url: location }))
    } else {
        Ok(None)
    }
}
