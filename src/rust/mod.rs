use crate::util::{escape_html_entities, size_humanize, CallbackSession, CALLBACK_SESSIONS};
use lazy_static::lazy_static;
use log::{error, info};
use std::collections::HashMap;
use teloxide::prelude::*;
use teloxide::requests::SendChatActionKind;
use teloxide::types::{
    CallbackQuery, ChatOrInlineMessage, InlineKeyboardButton, InlineKeyboardMarkup, ParseMode,
};
use tokio::sync::RwLock;

mod crates;
mod search;

pub async fn crate_information(
    cx: DispatcherHandlerCx<Message>,
    args: Vec<String>,
) -> ResponseResult<()> {
    const USAGE: &str = r#"<code>/crate [crate-name]</code>
Show information of a crate.

<code>[crate-name]</code>: the name of a crate"#;

    if args.is_empty() {
        cx.reply_to(USAGE)
            .parse_mode(ParseMode::HTML)
            .send()
            .await?;
    } else {
        cx.bot
            .send_chat_action(cx.chat_id(), SendChatActionKind::Typing)
            .send()
            .await?;
        let crate_name = &args[0];
        let information = {
            let result = crates::get_information(crate_name).await;
            match result {
                Err(e) => {
                    error!(
                        "Failed to get information of crate `{crate_name}`: {error}",
                        crate_name = crate_name,
                        error = e
                    );
                    return Ok(());
                }
                Ok(result) => result,
            }
        };
        if let Some(information) = information {
            info!("CrateInfo {{ Name = {} }}", crate_name);

            let authors = {
                let (primary_author, omitted) = information.owner.split_at(1);
                let mut authors = format!(
                    "<a href=\"{url}\">{name}</a>",
                    name = primary_author[0]
                        .name
                        .as_ref()
                        .unwrap_or(&"&lt;anonymous&gt;".to_string()),
                    url = primary_author[0].url
                );
                if !omitted.is_empty() {
                    authors.push_str(&format!(" and {} others", omitted.len()));
                }
                authors
            };

            let license = if let Some(license) = information.license {
                format!("{} License", license)
            } else {
                "No License".into()
            };

            let (updated_elapsed, created_elapsed) = {
                let now = chrono::Utc::now();
                (now - information.updated_at, now - information.created_at)
            };

            let keywords = if information.keywords.is_empty() {
                String::new()
            } else {
                format!(
                    "\n\n<b>Keywords</b>\n<i>{}</i>",
                    information.keywords.join(", ")
                )
            };

            let categories = if information.categories.is_empty() {
                String::new()
            } else {
                format!(
                    "\n\n<b>Categories</b>\n<i>{}</i>",
                    information.categories.join("\n")
                )
            };
            use chrono_humanize::HumanTime;
            let info_text = format!(
                r#"<b>{crate_name}</b> <i>{latest}</i> ({size}B) by {authors}
{license}

{description}{keywords}{categories}

⬇️{recent} downloads recently ({total} total)
📊{dependencies} dependencies ({dev_dependencies} for dev)
🕒 updated at {updated_at} ({updated_elapsed})
🕒 created at {created_at} ({created_elapsed})
"#,
                crate_name = information.name,
                latest = information.newest_version,
                size = size_humanize(information.crate_size),
                authors = authors,
                license = license,
                description = escape_html_entities(&information.description),
                updated_at = information.updated_at.format("%Y-%m-%d %Z"),
                created_at = information.created_at.format("%Y-%m-%d %Z"),
                recent = size_humanize(information.recent_downloads),
                total = size_humanize(information.downloads),
                dependencies = information.dependency_count,
                dev_dependencies = information.dev_dependency_count,
                keywords = keywords,
                categories = categories,
                updated_elapsed = HumanTime::from(updated_elapsed),
                created_elapsed = HumanTime::from(created_elapsed),
            );
            let markup = {
                let mut line = Vec::new();
                if let Some(homepage) = information.homepage {
                    let button = InlineKeyboardButton::url("🏠 Home".into(), homepage);
                    line.push(button);
                }
                let default_docs = format!("https://docs.rs/{}", crate_name);
                let button = InlineKeyboardButton::url(
                    "📚 Docs".into(),
                    information.documentation.unwrap_or(default_docs),
                );
                line.push(button);
                if let Some(repository) = information.repository {
                    let button = InlineKeyboardButton::url("📂 Repo".into(), repository);
                    line.push(button);
                }
                Some(InlineKeyboardMarkup {
                    inline_keyboard: vec![line],
                })
            };
            let message = cx.reply_to(info_text).parse_mode(ParseMode::HTML);
            if let Some(markup) = markup {
                message.reply_markup(markup).send().await?;
            } else {
                message.send().await?;
            }
        } else {
            let not_found = format!(
                "No crate `{crate_name}` has found",
                crate_name = crate_name.replace('`', "\\`")
            );
            cx.answer(&not_found)
                .parse_mode(ParseMode::MarkdownV2)
                .send()
                .await?;
        }
    }
    Ok(())
}

lazy_static! {
    static ref SEARCH_RESULT: RwLock<HashMap<(i64, i32), search::CrateDocument>> =
        RwLock::new(HashMap::new());
}

pub async fn search_crate(
    cx: DispatcherHandlerCx<Message>,
    args: Vec<String>,
) -> ResponseResult<()> {
    const USAGE: &str = r#"<code>/docs [path]</code>
Show online documentation with specified path in a crate.

<code>[path]</code>: the path to the item"#;

    if args.is_empty() {
        cx.reply_to(USAGE)
            .parse_mode(ParseMode::HTML)
            .send()
            .await?;
    } else {
        cx.bot
            .send_chat_action(cx.chat_id(), SendChatActionKind::Typing)
            .send()
            .await?;
        let path = &args[0];
        let document = {
            let result = search::get_document(path).await;
            match result {
                Err(e) => {
                    log::error!(
                        "Failed to get information with path `{path}`: {error}",
                        path = path,
                        error = e
                    );
                    return Ok(());
                }
                Ok(result) => result,
            }
        };
        if let Some(document) = document {
            use search::CrateDocument;
            info!("Docs {{ Path = {} }}", path);

            let inline_message = match &document {
                CrateDocument::Module {
                    path,
                    portability,
                    modules,
                    structs,
                    traits,
                    enums,
                    macros,
                    functions,
                    attributes,
                    consts,
                } => {
                    let portability_text = if let Some(portability) = portability {
                        format!("<u>({})</u>", portability)
                    } else {
                        String::new()
                    };
                    let module_text = format!(
                        "Module <code>{module}</code> {portability}",
                        module = path,
                        portability = portability_text
                    );
                    let markup = {
                        let mut inline_keyboard = Vec::new();
                        if !modules.is_empty() {
                            inline_keyboard.push(InlineKeyboardButton::callback(
                                "Modules".into(),
                                "modules".into(),
                            ));
                        }
                        if !structs.is_empty() {
                            inline_keyboard.push(InlineKeyboardButton::callback(
                                "Structs".into(),
                                "structs".into(),
                            ));
                        }
                        if !traits.is_empty() {
                            inline_keyboard.push(InlineKeyboardButton::callback(
                                "Traits".into(),
                                "traits".into(),
                            ));
                        }
                        if !enums.is_empty() {
                            inline_keyboard.push(InlineKeyboardButton::callback(
                                "Enums".into(),
                                "enums".into(),
                            ));
                        }
                        if !macros.is_empty() {
                            inline_keyboard.push(InlineKeyboardButton::callback(
                                "Macros".into(),
                                "macros".into(),
                            ));
                        }
                        if !functions.is_empty() {
                            inline_keyboard.push(InlineKeyboardButton::callback(
                                "Functions".into(),
                                "functions".into(),
                            ));
                        }
                        if !attributes.is_empty() {
                            inline_keyboard.push(InlineKeyboardButton::callback(
                                "Attributes".into(),
                                "attributes".into(),
                            ));
                        }
                        if !consts.is_empty() {
                            inline_keyboard.push(InlineKeyboardButton::callback(
                                "Consts".into(),
                                "modules".into(),
                            ));
                        }
                        InlineKeyboardMarkup {
                            inline_keyboard: inline_keyboard
                                .chunks(2)
                                .map(|chunk| chunk.to_owned())
                                .collect(),
                        }
                    };
                    cx.reply_to(module_text)
                        .parse_mode(ParseMode::HTML)
                        .reply_markup(markup)
                        .send()
                        .await?
                }
                CrateDocument::Function {
                    path,
                    definition,
                    portability,
                    description,
                    sections,
                } => {
                    let portability_text = if let Some(portability) = portability {
                        format!("<u>({})</u>", portability)
                    } else {
                        String::new()
                    };
                    let function_text = format!(
                        r#"Function <code>{function}</code> {portability}
{definition}

{description}"#,
                        function = path,
                        definition = definition,
                        portability = portability_text,
                        description = description,
                    );
                    let markup = {
                        let mut inline_keyboard = Vec::new();
                        for (i, (key, _)) in sections.iter().enumerate() {
                            inline_keyboard.push(vec![InlineKeyboardButton::callback(
                                key.clone(),
                                i.to_string(),
                            )]);
                        }
                        InlineKeyboardMarkup { inline_keyboard }
                    };
                    cx.reply_to(function_text)
                        .parse_mode(ParseMode::HTML)
                        .reply_markup(markup)
                        .send()
                        .await?
                }
                CrateDocument::Struct {
                    path,
                    definition,
                    portability,
                    description,
                    sections,
                    ..
                } => {
                    let portability_text = if let Some(portability) = portability {
                        format!("<u>({})</u>", portability)
                    } else {
                        String::new()
                    };
                    let struct_text = format!(
                        r#"Struct <code>{struct}</code> {portability}
{definition}

{description}"#,
                        struct = path,
                        definition = definition,
                        portability = portability_text,
                        description = description,
                    );
                    let markup = {
                        let mut inline_keyboard = vec![vec![
                            InlineKeyboardButton::callback("Methods".into(), "methods".into()),
                            InlineKeyboardButton::callback("Impls".into(), "impls".into()),
                        ]];
                        for (i, (key, _)) in sections.iter().enumerate() {
                            inline_keyboard.push(vec![InlineKeyboardButton::callback(
                                key.clone(),
                                i.to_string(),
                            )]);
                        }
                        InlineKeyboardMarkup { inline_keyboard }
                    };
                    cx.reply_to(struct_text)
                        .parse_mode(ParseMode::HTML)
                        .reply_markup(markup)
                        .send()
                        .await?
                }
                CrateDocument::Trait {
                    path,
                    definition,
                    portability,
                    description,
                    sections,
                    ..
                } => {
                    let portability_text = if let Some(portability) = portability {
                        format!("<u>({})</u>", portability)
                    } else {
                        String::new()
                    };
                    let trait_text = format!(
                        r#"Trait <code>{trait}</code> {portability}
{definition}

{description}"#,
                        trait = path,
                        definition = definition,
                        portability = portability_text,
                        description = description,
                    );
                    let markup = {
                        let mut inline_keyboard = vec![
                            vec![InlineKeyboardButton::callback(
                                "Required Methods".into(),
                                "rmethods".into(),
                            )],
                            vec![InlineKeyboardButton::callback(
                                "Provided Methods".into(),
                                "pmethods".into(),
                            )],
                            vec![InlineKeyboardButton::callback(
                                "Implementations".into(),
                                "impls".into(),
                            )],
                            vec![InlineKeyboardButton::callback(
                                "Implementors".into(),
                                "implors".into(),
                            )],
                        ];
                        for (i, (key, _)) in sections.iter().enumerate() {
                            inline_keyboard.push(vec![InlineKeyboardButton::callback(
                                key.clone(),
                                i.to_string(),
                            )]);
                        }
                        InlineKeyboardMarkup { inline_keyboard }
                    };
                    cx.reply_to(trait_text)
                        .parse_mode(ParseMode::HTML)
                        .reply_markup(markup)
                        .send()
                        .await?
                }
                CrateDocument::Method {
                    path,
                    definition,
                    portability,
                    description,
                    sections,
                } => {
                    let portability_text = if let Some(portability) = portability {
                        format!("<u>({})</u>", portability)
                    } else {
                        String::new()
                    };
                    let method_text = format!(
                        r#"Method <code>{method}</code> {portability}
{definition}

{description}"#,
                        method = path,
                        definition = definition,
                        portability = portability_text,
                        description = description,
                    );
                    let markup = {
                        let mut inline_keyboard = Vec::new();
                        for (i, (key, _)) in sections.iter().enumerate() {
                            inline_keyboard.push(vec![InlineKeyboardButton::callback(
                                key.clone(),
                                i.to_string(),
                            )]);
                        }
                        InlineKeyboardMarkup { inline_keyboard }
                    };
                    cx.reply_to(method_text)
                        .parse_mode(ParseMode::HTML)
                        .reply_markup(markup)
                        .send()
                        .await?
                }
                CrateDocument::TraitMethod {
                    path,
                    definition,
                    portability,
                    description,
                    sections,
                } => {
                    let portability_text = if let Some(portability) = portability {
                        format!("<u>({})</u>", portability)
                    } else {
                        String::new()
                    };
                    let method_text = format!(
                        r#"Trait Method <code>{method}</code> {portability}
{definition}

{description}"#,
                        method = path,
                        definition = definition,
                        portability = portability_text,
                        description = description,
                    );
                    let markup = {
                        let mut inline_keyboard = Vec::new();
                        for (i, (key, _)) in sections.iter().enumerate() {
                            inline_keyboard.push(vec![InlineKeyboardButton::callback(
                                key.clone(),
                                i.to_string(),
                            )]);
                        }
                        InlineKeyboardMarkup { inline_keyboard }
                    };
                    cx.reply_to(method_text)
                        .parse_mode(ParseMode::HTML)
                        .reply_markup(markup)
                        .send()
                        .await?
                }
            };
            {
                let mut lock = SEARCH_RESULT.write().await;
                lock.insert((inline_message.chat_id(), inline_message.id), document);
            }
            {
                let mut lock = CALLBACK_SESSIONS.write().await;
                lock.insert(
                    (inline_message.chat_id(), inline_message.id),
                    CallbackSession::Docs,
                );
            }
        } else {
            let not_found = format!("Could not find `{path}`", path = path.replace('`', "\\`"));
            cx.reply_to(&not_found)
                .parse_mode(ParseMode::MarkdownV2)
                .send()
                .await?;
        }
    }
    Ok(())
}

pub async fn search_crate_callback(cx: DispatcherHandlerCx<CallbackQuery>) -> ResponseResult<()> {
    let message = cx.update.message.as_ref().unwrap();
    let data = cx.update.data.as_ref().unwrap();

    let lock = SEARCH_RESULT.read().await;
    if let Some(search_result) = lock.get(&(message.chat_id(), message.id)) {
        use search::CrateDocument;
        match search_result {
            CrateDocument::Module {
                path,
                portability,
                modules,
                structs,
                traits,
                enums,
                macros,
                functions,
                attributes,
                consts,
            } => {
                info!("DocsCallback {{ Path = {}, Data = {} }}", path, data);
                let documents = match data.as_ref() {
                    "modules" => Some(modules),
                    "structs" => Some(structs),
                    "traits" => Some(traits),
                    "enums" => Some(enums),
                    "macros" => Some(macros),
                    "functions" => Some(functions),
                    "attributes" => Some(attributes),
                    "consts" => Some(consts),
                    _ => None,
                };
                if let Some(documents) = documents {
                    let portability_text = if let Some(portability) = portability {
                        format!("<u>({})</u>", portability)
                    } else {
                        String::new()
                    };
                    let documents_text =
                        documents
                            .iter()
                            .map(items_to_text)
                            .fold(String::new(), |mut acc, s| {
                                acc.push_str(&s);
                                acc.push('\n');
                                acc
                            });
                    let module_text = format!(
                        "Module <code>{module}</code> {portability}\n\n{documents}",
                        module = path,
                        portability = portability_text,
                        documents = documents_text,
                    );
                    cx.bot
                        .edit_message_text(
                            ChatOrInlineMessage::Chat {
                                chat_id: message.chat_id().into(),
                                message_id: message.id,
                            },
                            module_text,
                        )
                        .parse_mode(ParseMode::HTML)
                        .reply_markup(message.reply_markup().unwrap().clone())
                        .send()
                        .await?;
                }
            }
            CrateDocument::Function {
                path,
                definition,
                portability,
                sections,
                ..
            } => {
                info!("DocsCallback {{ Path = {}, Data = {} }}", path, data);
                if let Ok(index) = data.parse::<usize>() {
                    if let Some((title, document)) = sections.get(index) {
                        let portability_text = if let Some(portability) = portability {
                            format!("<u>({})</u>", portability)
                        } else {
                            String::new()
                        };
                        let function_text = format!(
                            r#"Function <code>{function}</code> {portability}
{definition}

<b>{title}</b>
{document}"#,
                            function = path,
                            definition = definition,
                            portability = portability_text,
                            title = title,
                            document = document,
                        );
                        cx.bot
                            .edit_message_text(
                                ChatOrInlineMessage::Chat {
                                    chat_id: message.chat_id().into(),
                                    message_id: message.id,
                                },
                                function_text,
                            )
                            .parse_mode(ParseMode::HTML)
                            .reply_markup(message.reply_markup().unwrap().clone())
                            .send()
                            .await?;
                    }
                }
            }
            CrateDocument::Struct {
                path,
                portability,
                sections,
                methods,
                implementations,
                ..
            } => {
                info!("DocsCallback {{ Path = {}, Data = {} }}", path, data);
                let section = data
                    .parse::<usize>()
                    .ok()
                    .and_then(|index| sections.get(index))
                    .cloned()
                    .or(match data.as_ref() {
                        "methods" => Some(("Methods".into(), methods.join("\n\n"))),
                        "impls" => Some(("Implementations".into(), implementations.join("\n\n"))),
                        _ => None,
                    });

                if let Some((title, document)) = section {
                    let portability_text = if let Some(portability) = portability {
                        format!("<u>({})</u>", portability)
                    } else {
                        String::new()
                    };

                    let struct_text = format!(
                        r#"Struct <code>{struct}</code> {portability}

<b>{title}</b>
{document}"#,
                        struct = path,
                        portability = portability_text,
                        title = title,
                        document = document,
                    );

                    cx.bot
                        .edit_message_text(
                            ChatOrInlineMessage::Chat {
                                chat_id: message.chat_id().into(),
                                message_id: message.id,
                            },
                            struct_text,
                        )
                        .parse_mode(ParseMode::HTML)
                        .reply_markup(message.reply_markup().unwrap().clone())
                        .send()
                        .await?;
                }
            }
            CrateDocument::Trait {
                path,
                portability,
                sections,
                required_methods,
                provided_methods,
                implementations,
                implementors,
                ..
            } => {
                info!("DocsCallback {{ Path = {}, Data = {} }}", path, data);
                let section = data
                    .parse::<usize>()
                    .ok()
                    .and_then(|index| sections.get(index))
                    .cloned()
                    .or(match data.as_ref() {
                        "rmethods" => {
                            Some(("Required Methods".into(), required_methods.join("\n\n")))
                        }
                        "pmethods" => {
                            Some(("Provided Methods".into(), provided_methods.join("\n\n")))
                        }
                        "impls" => Some((
                            "Foreign Implementations".into(),
                            implementations.join("\n\n"),
                        )),
                        "implors" => Some(("Implementors".into(), implementors.join("\n\n"))),
                        _ => None,
                    });

                if let Some((title, document)) = section {
                    let portability_text = if let Some(portability) = portability {
                        format!("<u>({})</u>", portability)
                    } else {
                        String::new()
                    };

                    let trait_text = format!(
                        r#"Trait <code>{trait}</code> {portability}

<b>{title}</b>
{document}"#,
                        trait = path,
                        portability = portability_text,
                        title = title,
                        document = document,
                    );

                    cx.bot
                        .edit_message_text(
                            ChatOrInlineMessage::Chat {
                                chat_id: message.chat_id().into(),
                                message_id: message.id,
                            },
                            trait_text,
                        )
                        .parse_mode(ParseMode::HTML)
                        .reply_markup(message.reply_markup().unwrap().clone())
                        .send()
                        .await?;
                }
            }
            CrateDocument::Method {
                path,
                definition,
                portability,
                sections,
                ..
            } => {
                info!("DocsCallback {{ Path = {}, Data = {} }}", path, data);
                if let Ok(index) = data.parse::<usize>() {
                    if let Some((title, document)) = sections.get(index) {
                        let portability_text = if let Some(portability) = portability {
                            format!("<u>({})</u>", portability)
                        } else {
                            String::new()
                        };
                        let method_text = format!(
                            r#"Method <code>{method}</code> {portability}
{definition}

<b>{title}</b>
{document}"#,
                            method = path,
                            definition = definition,
                            portability = portability_text,
                            title = title,
                            document = document,
                        );
                        cx.bot
                            .edit_message_text(
                                ChatOrInlineMessage::Chat {
                                    chat_id: message.chat_id().into(),
                                    message_id: message.id,
                                },
                                method_text,
                            )
                            .parse_mode(ParseMode::HTML)
                            .reply_markup(message.reply_markup().unwrap().clone())
                            .send()
                            .await?;
                    }
                }
            }
            CrateDocument::TraitMethod {
                path,
                definition,
                portability,
                sections,
                ..
            } => {
                info!("DocsCallback {{ Path = {}, Data = {} }}", path, data);
                if let Ok(index) = data.parse::<usize>() {
                    if let Some((title, document)) = sections.get(index) {
                        let portability_text = if let Some(portability) = portability {
                            format!("<u>({})</u>", portability)
                        } else {
                            String::new()
                        };
                        let method_text = format!(
                            r#"Trait Method <code>{method}</code> {portability}
{definition}

<b>{title}</b>
{document}"#,
                            method = path,
                            definition = definition,
                            portability = portability_text,
                            title = title,
                            document = document,
                        );
                        cx.bot
                            .edit_message_text(
                                ChatOrInlineMessage::Chat {
                                    chat_id: message.chat_id().into(),
                                    message_id: message.id,
                                },
                                method_text,
                            )
                            .parse_mode(ParseMode::HTML)
                            .reply_markup(message.reply_markup().unwrap().clone())
                            .send()
                            .await?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn items_to_text(item: &search::DocumentBriefItem) -> String {
    let name = if item.deprecated {
        format!("<s>{}</s>", item.name)
    } else {
        item.name.clone()
    };
    let portability_text = if let Some(portability) = &item.portability {
        format!("<u>({})</u>", portability)
    } else {
        String::new()
    };
    format!(
        "<code>{name}</code> {portability}\n{description}",
        name = name,
        portability = portability_text,
        description = item.description,
    )
}
