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
    const USAGE: &str = "<code>/crate [crate-name]</code>\n\
        Show information of a crate.\n\
        \n\
        <code>[crate-name]</code>: the name of a crate";

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
                "<b>{crate_name}</b> <i>{latest}</i> ({size}B) by {authors}\n\
                {license}\n\
                \n\
                {description}{keywords}{categories}\n\
                \n\
                ⬇️{recent} downloads recently ({total} total)\n\
                📊{dependencies} dependencies ({dev_dependencies} for dev)\n\
                🕒 updated at {updated_at} ({updated_elapsed})\n\
                🕒 created at {created_at} ({created_elapsed})",
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
    const USAGE: &str = "<code>/docs [path]</code>\n\
        Show online documentation with specified path in a crate.\n\
        \n\
        <code>[path]</code>: the path to the item";

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
            info!("Docs {{ Path = {} }}", path);

            let portability_text = if let Some(portability) = &document.portability_note {
                format!("\n<i>{}</i>", portability)
            } else {
                String::new()
            };

            let stability_text = if let Some(stability) = &document.stability_note {
                format!("\n<i>{}</i>", stability)
            } else {
                String::new()
            };

            let deprecated_text = if document.deprecated {
                "<b>Deprecated</b>"
            } else {
                ""
            };

            let definition_text = if let Some(definition) = &document.definition {
                format!("\n{}", definition)
            } else {
                String::new()
            };

            let text = format!(
                "{title} {deprecated}{portability}{stability}{definition}\n\
                \n\
                {description}",
                title = document.title,
                deprecated = deprecated_text,
                portability = portability_text,
                stability = stability_text,
                definition = definition_text,
                description = document.description,
            );
            let markup = InlineKeyboardMarkup {
                inline_keyboard: document
                    .sections
                    .iter()
                    .enumerate()
                    .map(|(i, (heading, _))| {
                        vec![InlineKeyboardButton::callback(
                            heading.clone(),
                            i.to_string(),
                        )]
                    })
                    .collect(),
            };
            let message = cx
                .reply_to(text)
                .parse_mode(ParseMode::HTML)
                .reply_markup(markup)
                .send()
                .await?;
            {
                let mut lock = SEARCH_RESULT.write().await;
                lock.insert((message.chat_id(), message.id), document);
            }
            {
                let mut lock = CALLBACK_SESSIONS.write().await;
                lock.insert((message.chat_id(), message.id), CallbackSession::Docs);
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
    if let Some(document) = lock.get(&(message.chat_id(), message.id)) {
        if let Some((heading, article)) = data
            .parse::<usize>()
            .ok()
            .and_then(|i| document.sections.get(i))
        {
            info!("Docs {{ Title = {}, Data = {} }}", document.title, data);
            let portability_text = if let Some(portability) = &document.portability_note {
                format!("\n<i>{}</i>", portability)
            } else {
                String::new()
            };

            let stability_text = if let Some(stability) = &document.stability_note {
                format!("\n<i>{}</i>", stability)
            } else {
                String::new()
            };

            let deprecated_text = if document.deprecated {
                "<b>Deprecated</b>"
            } else {
                ""
            };

            let definition_text = if let Some(definition) = &document.definition {
                format!("\n{}", definition)
            } else {
                String::new()
            };

            let text = format!(
                "{title} {deprecated}{portability}{stability}{definition}\n\
                \n\
                <b>{heading}</b>\n\
                {article}\n",
                title = document.title,
                deprecated = deprecated_text,
                portability = portability_text,
                stability = stability_text,
                definition = definition_text,
                heading = heading,
                article = article_to_text(article),
            );

            cx.bot
                .edit_message_text(
                    ChatOrInlineMessage::Chat {
                        chat_id: message.chat_id().into(),
                        message_id: message.id,
                    },
                    text,
                )
                .parse_mode(ParseMode::HTML)
                .reply_markup(message.reply_markup().unwrap().clone())
                .send()
                .await?;
        }
    }
    Ok(())
}

fn article_to_text(item: &search::Article) -> String {
    match item {
        search::Article::Text(text) => text.clone(),
        search::Article::SubDocuments(documents) => documents
            .iter()
            .map(|subdocument| {
                let deprecated_text = if subdocument.deprecated {
                    "<b>Deprecated</b>"
                } else {
                    ""
                };

                let portability_text = if let Some(portability) = &subdocument.portability_note {
                    format!("\n<i>{}</i>", portability)
                } else {
                    String::new()
                };

                let stability_text = if let Some(stability) = &subdocument.stability_note {
                    format!("\n<i>{}</i>", stability)
                } else {
                    String::new()
                };

                let summary_text = if let Some(summary) = &subdocument.summary {
                    format!("\n{}", summary)
                } else {
                    String::new()
                };

                format!(
                    "<code>{name}</code> {deprecated}{portability}{stability}{summary}\n",
                    name = subdocument.name,
                    deprecated = deprecated_text,
                    portability = portability_text,
                    stability = stability_text,
                    summary = summary_text,
                )
            })
            .collect(),
    }
}
