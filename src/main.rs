use teloxide::prelude::*;
use teloxide::types::CallbackQuery;
use teloxide::utils::command::BotCommand;

mod rust;
pub mod util;

fn main() {
    use tokio::runtime::*;
    log4rs::init_file("config/log4rs.yml", Default::default()).unwrap();
    let mut rt = Runtime::new().expect("Failed to create task runtime");
    rt.block_on(run());
}

async fn run() {
    let bot = Bot::from_env();
    let information = bot
        .get_me()
        .send()
        .await
        .expect("Failed to get the bot information.");
    let username = information.user.username.unwrap();
    Dispatcher::new(bot)
        .messages_handler(move |rx: DispatcherHandlerRx<Message>| {
            rx.commands(username)
                .for_each_concurrent(None, command_handler)
        })
        .callback_queries_handler(|rx: DispatcherHandlerRx<CallbackQuery>| {
            rx.for_each_concurrent(None, callback_handler)
        })
        .dispatch()
        .await;
}

async fn command_handler(
    (cx, command, args): (DispatcherHandlerCx<Message>, Command, Vec<String>),
) {
    match command {
        Command::Crate => {
            rust::crate_information(cx, args).await.log_on_error().await;
        }
        Command::Help => {
            cx.reply_to(Command::descriptions())
                .send()
                .await
                .log_on_error()
                .await;
        }
        Command::Docs => {
            rust::search_crate(cx, args).await.log_on_error().await;
        }
    };
}

async fn callback_handler(query: DispatcherHandlerCx<CallbackQuery>) {
    if let CallbackQuery {
        message: Some(message),
        data: Some(_),
        ..
    } = &query.update
    {
        let session = {
            let lock = util::CALLBACK_SESSIONS.read().await;
            lock.get(&(message.chat_id(), message.id)).cloned()
        };
        if let Some(session) = session {
            use util::CallbackSession;
            match session {
                CallbackSession::Docs => {
                    rust::search_crate_callback(query)
                        .await
                        .log_on_error()
                        .await;
                }
            }
        }
    }
}

#[derive(BotCommand)]
#[command(rename = "lowercase")]
enum Command {
    #[command(description = "show help message")]
    Help,
    #[command(description = "show the information of a crate")]
    Crate,
    #[command(description = "show the documentation of a crate item")]
    Docs,
}
