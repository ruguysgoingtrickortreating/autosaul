use std::collections::HashSet;
use std::time::Duration;
use itertools::Itertools;
use crate::{Context, Error};
use poise::{serenity_prelude as serenity, CreateReply};
use poise::serenity_prelude::EditMessage;
use rand::prelude::{IndexedRandom, IteratorRandom, SliceRandom};
use tokio::sync::mpsc;
use tokio::time::sleep;

#[derive(Default)]
pub struct Games {
    pub horse_racing: Option<crate::gambling::HorseRacingData>,
    pub imposter: Option<ImposterGameData>,
    pub blackjack: crate::gambling::BlackjackData,
}

pub struct ImposterGameData {
    words: Vec<String>,
    participants: HashSet<serenity::UserId>,
    initiator: serenity::UserId,
    join_input:  Option<mpsc::UnboundedSender<serenity::UserId>>,
}

#[poise::command(prefix_command,category = "games", aliases("impostor"))]
pub async fn imposter(ctx: Context<'_>, #[rest] wordlist: String) -> Result<(), Error> {
    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("not in a server").await?;
        return Ok(());
    };

    let mut games = ctx.data().active_games.lock().await;
    let gg = if let Some(gg) = games.get_mut(&guild_id) {
        if gg.imposter.is_some() {
            ctx.say("an imposter game is already running in this server").await?;
            return Ok(());
        }
        gg
    } else {
        games.insert(guild_id, Games::default());
        games.get_mut(&guild_id).unwrap()
    };

    let word_list = wordlist.split(",").map(|word| word.trim().to_string()).collect::<Vec<String>>();

    match word_list.len() {
        0 => {
            ctx.say("tried to make an imposter game with 0 valid words").await?;
            return Ok(());
        }
        1 => {
            ctx.say("⚠️ WARNING: this imposter game only has 1 word. you probably messed up the formatting. words must be comma separated").await?;
        }
        _ => ()
    };

    let mut hs = HashSet::new();
    hs.insert(ctx.author().id);

    let (send_join, mut reciever) = mpsc::unbounded_channel();

    let imp_data = ImposterGameData {
        words: word_list,
        participants: hs,
        initiator: ctx.author().id,
        join_input: Some(send_join),
    };

    gg.imposter = Some(imp_data);
    drop(games);

    let mut message = ctx.send(CreateReply::default().content(format!("**imposter game**\nplayers: {}",ctx.author().name))).await?.into_message().await?;

    while let Some(response) = reciever.recv().await {
        let mut games = ctx.data().active_games.lock().await;
        if let Some(gg) = games.get_mut(&guild_id) && let Some(imposter) = &mut gg.imposter {
            imposter.participants.insert(response);
            let users = imposter.participants.clone().into_iter();
            drop(games);
            let mut users_str = vec![];
            for user in users {
                users_str.push(user.to_user(ctx).await?.name);
            }
            message.edit(&ctx, EditMessage::new().content(format!("**imposter game**\nplayers: {}",users_str.join(", ")))).await?;
        }
    }
    Ok(())
}

#[poise::command(prefix_command,category = "games")]
pub async fn join(ctx: Context<'_>) -> Result<(), Error> {
    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("not in a server").await?;
        return Ok(());
    };

    let mut games = ctx.data().active_games.lock().await;
    let Some(gg) = games.get_mut(&guild_id) else {
        ctx.say("no imposter games active").await?;
        return Ok(());
    };
    let Some(ref mut data) = gg.imposter else {
        let r = ctx.say("no imposter games active").await?.into_message().await?;
        sleep(Duration::from_secs(4)).await;
        r.delete(&ctx).await?;
        ctx.msg.delete(&ctx).await?;
        return Ok(());
    };
    let Some(sender) = &data.join_input else {
        ctx.say("imposter game not taking joins").await?;
        return Ok(());
    };

    let already_in = data.participants.contains(&ctx.author().id);

    if already_in {
        let r = ctx.say("you already joined this imposter game!").await?.into_message().await?;
        sleep(Duration::from_secs(4)).await;
        r.delete(&ctx).await?;
        ctx.msg.delete(&ctx).await?;
        return Ok(());
    } else {
        sender.send(ctx.author().id)?;
        ctx.msg.delete(&ctx).await?;
    }

    Ok(())
}

#[poise::command(prefix_command,category = "games")]
pub async fn start(ctx: Context<'_>) -> Result<(), Error> {
    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("not in a server").await?;
        return Ok(());
    };

    let mut games = ctx.data().active_games.lock().await;
    let Some(gg) = games.get_mut(&guild_id) else {
        ctx.say("no imposter games active").await?;
        return Ok(());
    };
    let Some(ref mut data) = gg.imposter else {
        let r = ctx.say("no imposter games active").await?.into_message().await?;
        sleep(Duration::from_secs(4)).await;
        r.delete(&ctx).await?;
        ctx.msg.delete(&ctx).await?;
        return Ok(());
    };

    if data.initiator != ctx.author().id {
        ctx.say("you didn't start this imposter game").await?;
        return Ok(());
    }

    if data.join_input.is_some() {
        data.join_input = None;
    }

    let word = data.words.choose(&mut rand::rng()).unwrap();
    let imp = data.participants.iter().choose(&mut rand::rng()).unwrap();
    let mut users = vec![];

    for id in &data.participants {
        let user = id.to_user(ctx).await?;
        if id == imp {
            user.direct_message(ctx, serenity::CreateMessage::new().content("You are the imposter!")).await?;
        } else {
            user.direct_message(ctx, serenity::CreateMessage::new().content(word)).await?;
        }
        users.push(user.name);
    }

    users.shuffle(&mut rand::rng());
    ctx.say(users.join(", ")).await?;

    Ok(())
}

#[poise::command(prefix_command,category = "games")]
pub async fn stop_imposter(ctx: Context<'_>) -> Result<(), Error> {
    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("not in a server").await?;
        return Ok(());
    };

    let mut games = ctx.data().active_games.lock().await;
    let Some(gg) = games.get_mut(&guild_id) else {
        ctx.say("no imposter games active").await?;
        return Ok(());
    };

    gg.imposter = None;

    ctx.say("stopped imposter game maybe idk").await?;
    Ok(())
}
