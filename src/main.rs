use colored::Colorize;
use lavalink_rs::{
    client::LavalinkClient, model::events, node::NodeBuilder, prelude::NodeDistributionStrategy,
};
use poise::serenity_prelude::{self as serenity, CacheHttp};
use std::collections::HashMap;
use std::sync::LazyLock;

use rusqlite::Connection;
use songbird::SerenityInit;
use tokio::sync::Mutex;

mod commands;
use commands::*;

pub mod image_edit;

include!("../cards_map");

static HORSE_NAMES: [&'static str; 35] = [
    "Rick Ross",
    "SpongeBob",
    "Cheeto",
    "Netanyahu",
    "Mi Bombo",
    "RobTop",
    "mustard blud",
    "anna yeast",
    "diddly blud",
    "pea tear griffin",
    "Mega Knight",
    "Mason Troy Adams",
    "Quagmire",
    "Domer",
    "Chickawaga",
    "Triple T",
    "Charlie Kirk",
    "chinese potion",
    "nettspend",
    "Supercell",
    "Zoink",
    "Mr.Griddy",
    "Zheng He",
    "Bubba",
    "Little Saint James",
    "Bob l'eponge",
    "osa mason",
    "Muzammil",
    "Horse",
    "Racing",
    "Kidslookintouchable",
    "Coffeesmile1117",
    "Epstein",
    "adrian",
    "David",
];

static REQ_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| reqwest::Client::new());
struct Data {
    rand_words: Vec<&'static str>,
    pub lavalink: LavalinkClient,
    active_games: Mutex<HashMap<serenity::GuildId, games::Games>>,
}

// struct HttpKey;

// impl serenity::prelude::TypeMapKey for HttpKey {
//     type Value = HttpClient;
// }

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::PrefixContext<'a, Data, Error>;

#[tokio::main]
async fn main() {
    saulimages::init("autosaul");

    let response = reqwest::get("https://www.mit.edu/~ecprice/wordlist.10000").await;
    let response_utf;
    let mut rand_words: Vec<&'static str> = vec!["couldn't fetch random word list"];
    match response {
        Ok(val) => {
            response_utf = val
                .text()
                .await
                .expect("Got random word list but could not read as text");
            rand_words = response_utf
                .lines()
                .map(|line| Box::leak(line.to_string().into_boxed_str()) as &'static str)
                .collect();
        }
        Err(err) => eprintln!("COULD NOT FETCH RANDOM WORD LIST: {:?}", err),
    };

    if let Ok(db) = Connection::open("saul.db") {
        if let Err(err) = db.execute(
            "create table if not exists saul (
                id integer primary key,
                discord_id integer unique,
                agarthereum integer not null default 0
            )",
            [],
        ) {
            eprintln!("COULD NOT CREATE DATABASE TABLE: {}", err)
        }
    }

    let active_games: Mutex<HashMap<serenity::GuildId, games::Games>> = Mutex::new(HashMap::new());

    let token = include_str!("../token.txt");
    let intents = serenity::GatewayIntents::non_privileged()
        | serenity::GatewayIntents::MESSAGE_CONTENT
        | serenity::GatewayIntents::GUILDS;

    let framework = poise::Framework::<Data, Error>::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                cmds::help(),
                cmds::saul(),
                cmds::getmessage(),
                cmds::ban(),
                cmds::deletesince(),
                cmds::restart(),
                cmds::tickle(),
                cmds::streamtest(),
                cmds::invite(),
                cmds::cards_named_finger(),
                audio::play(),
                audio::stop(),
                audio::search(),
                audio::clear(),
                audio::skip(),
                audio::queue(),
                audio::nowplaying(),
                audio::skipto(),
                audio::rewind(),
                audio::fastforward(),
                audio::pause(),
                audio::resume(),
                audio::set(),
                audio::remove(),
                audio::joe(),
                gambling::balance(),
                gambling::give(),
                gambling::horseracing(),
                gambling::wager(),
                gambling::blackjack(),
                gambling::coalmines(),
                gambling::chinesesweatshop(),
                games::imposter(),
                games::join(),
                games::start(),
                games::stop_imposter(),
                images::chain(),
                images::caption(),
                images::pugsley(),
                images::rio_de_janeiro(),
                images::papyrus(),
                images::burn(),
            ], /////////////////////////////////////////////
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("!".into()),
                ..Default::default()
            },
            event_handler: |ctx, event, framework, data| {
                Box::pin(async move {
                    match event {
                        serenity::FullEvent::Message { new_message } => {
                            let guild_name = if let Some(gid) = new_message.guild_id {
                                gid.to_string()
                            } else {
                                "none".to_string()
                            };

                            println!(
                                "[{guild_name}] {}: {}",
                                new_message.author.name.bright_green(),
                                new_message.content
                            );
                            // for attachment in &new_message.attachments {
                            //     println!("          - {}",attachment.content_type.as_ref().unwrap());
                            // }

                            match new_message.content.as_str() {
                                "😉" => {
                                    let rand_range = rand::random_range(0..data.rand_words.len());
                                    new_message
                                        .channel_id
                                        .say(&ctx.http, &*data.rand_words[rand_range])
                                        .await?;
                                }
                                "hit" => {
                                    println!("hitting");
                                    let mut games = data.active_games.lock().await;
                                    println!("awaited");
                                    if let Some(guild_id) = new_message.guild_id
                                    { dbg!(guild_id); if let Some(gg) = games.get_mut(&guild_id)
                                    { println!("has games");if let Some(snd) = gg.blackjack.active_games.get(&new_message.author.id.get()) {
                                        println!("getting sendy");
                                        snd.send(true).await;
                                    }}}
                                }
                                "stay" | "stand" => {
                                    println!("standing");
                                    let mut games = data.active_games.lock().await;
                                    println!("awaited");
                                    if let Some(guild_id) = new_message.guild_id &&
                                    let Some(gg) = games.get_mut(&guild_id) && 
                                    let Some(snd) = gg.blackjack.active_games.get(&new_message.author.id.get()) {
                                        println!("getting sendy");
                                        snd.send(false).await;
                                    }
                                }
                                _ => ()
                            }
                        }
                        serenity::FullEvent::VoiceStateUpdate { old, new } => {
                            let Some(oldstatus) = old else {
                                return Ok(());
                            };
                            let channel = oldstatus
                                .channel_id
                                .unwrap()
                                .to_channel(&ctx.http())
                                .await
                                .unwrap();
                            if let serenity::Channel::Guild(channel) = channel {
                                let members = channel.members(ctx.cache().unwrap())?;
                                if members.len() == 1 {
                                    if members[0].user.id == framework.bot_id {
                                        let manager = songbird::get(ctx).await.unwrap().clone();
                                        let guild_id = new.guild_id.unwrap();

                                        data.lavalink.clone().delete_player(guild_id).await?;

                                        if manager.get(guild_id).is_some() {
                                            manager.remove(guild_id).await?;
                                        }
                                        println!(
                                            "[{}] {}",
                                            guild_id
                                                .name(ctx.cache.clone())
                                                .unwrap_or(guild_id.to_string()),
                                            "everyone left, leaving voice channel..".purple()
                                        );
                                    }
                                }
                            }
                        }
                        _ => (),
                    }

                    Ok(())
                })
            },
            pre_command: |ctx| {
                Box::pin(async move {
                    let guild_name = if let Some(g) = ctx.guild() {
                        g.name.clone()
                    } else {
                        "none".to_string()
                    };
                    println!(
                        "[{}] executing command \"{}\"",
                        guild_name,
                        ctx.command().qualified_name.bright_red()
                    );
                })
            },
            ..Default::default()
        })
        .setup(|ctx, ready, _framework| {
            Box::pin(async move {
                let events = events::Events {
                    // raw: Some(audio_events::raw_event),
                    ready: Some(audio_events::ready_event),
                    track_start: Some(audio_events::track_start),
                    ..Default::default()
                };
                let node_local = NodeBuilder {
                    hostname: "localhost:2333".to_string(),
                    is_ssl: false,
                    events: events::Events::default(),
                    password: "youshallnotpass".to_string(),
                    user_id: lavalink_rs::model::UserId(ctx.cache.current_user().id.into()),
                    session_id: None,
                };
                let client = LavalinkClient::new(
                    events,
                    vec![node_local],
                    NodeDistributionStrategy::round_robin(),
                )
                .await;

                println!("Created bot as {}", ready.user.name.bright_green());
                // poise::builtins::register_globally(ctx, &framework.options().commands)
                Ok(Data {
                    rand_words: rand_words,
                    lavalink: client,
                    active_games: active_games,
                })
            })
        })
        .build();
    let mut client = match serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .register_songbird()
        // .type_map_insert::<HttpKey>(HttpClient::new())
        .await
    {
        Ok(c) => c,
        Err(e) => panic!("unable to start bot: {}", e),
    };

    client.start().await.unwrap()
}
