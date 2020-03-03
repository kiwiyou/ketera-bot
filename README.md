# Ketera

**Ketera** is a telegram bot built with ❤️ for Rust.
It features search on [crates.io](https://crates.io) and [docs.rs](https://docs.rs)

## Features
- `/crate` - browse crate information
- `/docs` - look up in the docs.rs documentation

## Inviting the public bot 
Ketera is served by [@KeteraBot.](https://t.me/KeteraBot)
You can invite the bot to your chat and make use of it.

**Note:** It is not being hosted 24/7 currently.

## Hosting the bot on your own
- Prerequisites
    - Rust 1.39.0 or higher
    - Create your bot and get the token by talking to [@BotFather](https://t.me/BotFather)
- Clone the repository
```bash
git clone https://github.com/kiwiyou/ketera-bot.git
cd ketera-bot
```
- Set the environmental variable
```bash
export TELOXIDE_TOKEN=<your bot token here>
```
- Build and run it
```bash
cargo build --release
cargo run --release
# Or you can execute the binary directly
./target/release/ketera-bot
```
You can customize your log system by modifying [config/log4rs.yml.](https://github.com/kiwiyou/ketera-bot/blob/master/config/log4rs.yml)
Find more details about log4rs configuration [here.](https://github.com/estk/log4rs)

## Contribution
Bug reports and code reviews are welcome. Feel free to send pull requests.
If you are going to request a new feature, please post an issue first.