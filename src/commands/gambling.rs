use std::{collections::{BTreeMap, HashMap}, fmt::Write, panic::AssertUnwindSafe, time::Duration};

use itertools::Itertools;
use rand::seq::IndexedRandom;
use rusqlite::Connection;
use colored::Colorize;

use poise::{CreateReply, serenity_prelude::{self as serenity, Color, CreateActionRow, CreateButton, CreateEmbed, CreateInteractionResponse, CreateInteractionResponseMessage, CreateMessage, EditMessage, FutureExt, Mentionable, User, futures::StreamExt}};
use tokio::{sync::{Mutex, mpsc}, time::{Instant, sleep, timeout}};
use crate::{Context,Error};


#[derive(Default)]
pub struct HorseRacingData {
    betting: Option<Instant>,
    wager_input: Option<mpsc::Sender<(User,usize)>>,
    // wagers: BTreeMap<serenity::UserId, (String, usize)>,
    bets: BTreeMap<&'static str, [Vec<(User,usize)>;3]>,
    pool: usize,
    playercount: usize,
}

async fn _set_db_account(ctx:&Context<'_>, id:i64) -> Result<rusqlite::Connection, rusqlite::Error> {
    let db = match Connection::open("saul.db") {
        Ok(db) => db,
        Err(err) => {
            ctx.say(format!("error opening database: {}",err)).await.unwrap();
            return Err(err);
        }
    };

    if let Err(err) = db.execute(
        "insert or ignore into saul (discord_id, agarthereum)
        values (?1, 200)",
        [i64::from(id)]
    ) {
        ctx.say(format!("error writing to database: {}",err)).await.unwrap();
        return Err(err);
    };
    Ok(db)
}

#[poise::command(prefix_command,category = "gambling")]
pub async fn balance(ctx:Context<'_>, user:Option<serenity::User>) -> Result<(),Error> {
    let id = i64::from(if let Some(u) = &user {u.id} else {ctx.author().id});
    let db = _set_db_account(&ctx, id).await?;

    let amount: i64 = db.query_row("select id, agarthereum from saul where discord_id = ?1",
                [id],
                |row| Ok(row.get(1)?))?;

    if let Some(u) = user {
        ctx.say(format!("{} has {} agarthereum 🪙",u.name,amount)).await?;
    } else {
        ctx.say(format!("you have {} agarthereum 🪙",amount)).await?;
    }
    Ok(())
}

#[poise::command(prefix_command,category = "gambling")]
pub async fn give(ctx:Context<'_>, user:serenity::User, amount:i64) -> Result<(), Error> {

    if ctx.author().id != 640722508093325342 {
        ctx.say("you don't have permission to do that").await?;
        return Ok(())
    }

    let db = match Connection::open("saul.db") {
        Ok(db) => db,
        Err(err) => {
            ctx.say(format!("error opening database: {}",err)).await?;
            return Ok(());
        }
    };
    if let Err(err) = db.execute(
        "INSERT INTO saul (discord_id, agarthereum)
        VALUES (?1, ?2)
        ON CONFLICT(discord_id) DO UPDATE SET agarthereum = agarthereum + excluded.agarthereum",
        [i64::from(user.id),amount]
    ) {     ctx.say(format!("error writing to database: {}",err)).await?;
            return Ok(());};

    let balance: i64 = db.query_row("select id, agarthereum from saul where discord_id = ?1",
                [i64::from(ctx.author().id)],
                |row| Ok(row.get(1)?))?;

    ctx.say(format!("{} now has {} agarthereum",user.name,balance)).await?;

    Ok(())
}

#[poise::command(prefix_command,category = "gambling")]
pub async fn wager(ctx:Context<'_>, amount:usize) -> Result<(), Error> {
    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("not in a server").await?;
        return Ok(());
    };

    let mut games = ctx.data().active_games.lock().await;
    let Some(gg) = games.get_mut(&guild_id) else {
        ctx.say("no horse racing matches active").await?;
        return Ok(());
    };
    let Some(ref mut data) = gg.horse_racing else {
        let r = ctx.say("no horse racing matches active").await?.into_message().await?;
        sleep(Duration::from_secs(4)).await;
        r.delete(&ctx).await?;
        ctx.msg.delete(&ctx).await?;
        return Ok(());
    };
    if data.wager_input.is_none() || data.wager_input.as_mut().unwrap().is_closed() {
        let r = ctx.say("horse race not accepting wagers").await?.into_message().await?;
        sleep(Duration::from_secs(4)).await;
        r.delete(&ctx).await?;
        ctx.msg.delete(&ctx).await?;
        return Ok(());
    }

    let db = _set_db_account(&ctx, i64::from(ctx.author().id)).await?;

    let balance: i64 = db.query_row("select id, agarthereum from saul where discord_id = ?1",
                [i64::from(ctx.author().id)],
                |row| Ok(row.get(1)?))?;
    
    if balance < amount as i64 {
        let m = ctx.channel_id().send_message(&ctx,CreateMessage::new().content("not enough agarthereum").reference_message(ctx.msg)).await?;
        sleep(Duration::from_secs(4)).await;
        m.delete(&ctx).await?;
    } else {
        data.wager_input.as_mut().unwrap().send((ctx.author().clone(),amount)).await?;
        db.execute("UPDATE saul SET agarthereum = agarthereum - ?2 WHERE discord_id = ?1",[i64::from(ctx.author().id),amount as i64])?;
    }

    ctx.msg.delete(&ctx).await?;
    
    Ok(())
}

async fn _hedge_bets_horse(ctx:&Context<'_>, horses: &mut BTreeMap<&'static str, usize>, message:&mut serenity::Message, horses_string:String) -> Result<(), Error> {
    let mut games = ctx.data().active_games.lock().await;
    let horseracing = games.get_mut(&ctx.guild_id().unwrap()).unwrap().horse_racing.as_mut().unwrap();
    horseracing.bets = horses.iter().map(|(&k,_)| (k,[vec![],vec![],vec![]])).collect();
    horseracing.betting = Some(Instant::now());

    let (send_wager, mut receiver) = mpsc::channel(100);
    horseracing.wager_input = Some(send_wager);
    drop(games);

    let components:Vec<CreateActionRow> = horses.iter().map(|(&x,_)| CreateButton::new(x).label(x)).chunks(5).into_iter().map(|x|CreateActionRow::Buttons(x.collect())).collect();

    // let mut wagers_msg = ctx.send(CreateReply {
    //     embeds: vec![CreateEmbed::new()
    //         .title("wagers")
    //         .color(Color::DARK_GREEN)
    //         .field("", "!wager [amount]  to wager money", false)],
    //     components: Some(components),
    //     ..Default::default()
    // }).await?.into_message().await?;

    message.edit(&ctx, EditMessage::new().components(components).add_embeds(vec![CreateEmbed::new()
        .title("horse racing")
        .color(Color::DARK_ORANGE)
        .field("", "-------------------------------------------------------------------------------------", false)
        .field("", &horses_string, false),CreateEmbed::new()
        .title("wagers")
        .color(Color::DARK_GREEN)
        .field("", "!wager [amount] to wager money", false)])).await?;
    
    let picks_mutex:Mutex<BTreeMap<serenity::UserId,(User, usize, Vec<String>)>> = Default::default();
    let msg_clone = message.clone();
    let mut pool: usize = 0;

    tokio::join!(
        async { // COUNTDOWN
            let ins = Instant::now();
            let mut time:u64 = 0;
            loop {
                let elapsed = ins.elapsed().as_secs();
                if elapsed > time {
                    time = elapsed;
                    let h_embed = CreateEmbed::new()
                        .title("horse racing")
                        .color(Color::DARK_ORANGE)
                        .field("", format!("🏁*you have **{}** seconds: run !wager [amount] to stake money, then bet on 3 horses*",30-time), false)
                        .field("", &horses_string, false);

                    let picks = picks_mutex.lock().await;
                    let m = if picks.is_empty() {
                        "!wager [amount] to wager money".to_string()
                    } else {
                        picks.iter().map(|(_,(k,n,v))|format!("🪙{} - {}: {}",n,k.name,v.join(", "))).collect::<Vec<String>>().join("\n")
                    };
                    drop(picks);
                    let w_embed = CreateEmbed::new()
                        .title("wagers")
                        .color(Color::DARK_GREEN)
                        .field("", m, false);
                    let _ = message.edit(&ctx, EditMessage::new().embeds(vec![h_embed,w_embed])).await;
                    if time >= 30 {
                        break;
                    }
                }
            }
        },
        async { // WAGER RECEIVER
            let timeout = sleep(Duration::from_secs(30));
            tokio::pin!(timeout);

            println!("recieving wagers");
            while let Some(response) = tokio::select! {
                () = &mut timeout => None,
                maybe_message = receiver.recv() => maybe_message,
            } {
                let name = response.0.name.clone();
                let ins = Instant::now();
                println!("awaiting picks in {}: {} {}ms","wager responder".to_string().red(),name.black(),ins.elapsed().as_millis());

                let mut picks = picks_mutex.lock().await;
                println!("accessed picks {}: {} {}ms","wager responder".to_string().red(),name.black(),ins.elapsed().as_millis());
                picks.insert(response.0.id,(response.0, response.1, vec![]));
                // println!("awaiting discord edit {}: {} {}ms","wager responder".to_string().red(),name.black(),ins.elapsed().as_millis());
                // let m = picks.iter().map(|(_,(k,n,v))|format!("🪙{} - {}: {}",n,k.name,v.join(", "))).collect::<Vec<String>>().join("\n");
                drop(picks);
                println!("dropped picks in {}: {} {}ms","wager responder".to_string().red(),name.black(),ins.elapsed().as_millis());
                // wagers_msg_clone.edit(&ctx, EditMessage::new().embed(CreateEmbed::new()
                //             .title("wagers")
                //             .color(Color::DARK_GREEN)
                //             .field("", m, false))).await.unwrap();
                // println!("finished discord edit {}: {} {}ms","wager responder".to_string().red(),name.black(),ins.elapsed().as_millis());
                pool += response.1;
            };
            println!("done recieving wagers");
        },
        async { // INTERACTION HANDLER
            let mut interaction_stream = msg_clone.await_component_interaction(&ctx.serenity_context().shard)
                .timeout(Duration::from_secs(30)).stream();

            println!("responding to interactions");
            while let Some(interaction) = interaction_stream.next().await {
                let name = interaction.user.name.clone();
                let horsename = interaction.data.custom_id.clone();
                let ins = Instant::now();
                println!("awaiting picks in {}: {} {}ms","interaction handler".to_string().blue(),format!("{} - {}",name,&horsename).black(),ins.elapsed().as_millis());
                let mut picks = picks_mutex.lock().await;
                println!("accessed picks in {}: {} {}ms","interaction handler".to_string().blue(),format!("{} - {}",name,&horsename).black(),ins.elapsed().as_millis());
                let Some(horse) = picks.get_mut(&interaction.user.id) else {
                    let _ = interaction.create_response(&ctx, CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new().content("you haven't placed a !wager [amount] yet").ephemeral(true))).await;
                    continue;
                };
                if horse.2.len() >= 3 {
                    let _ = interaction.create_response(&ctx, CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new().content("you already bet on 3 horses").ephemeral(true))).await;
                    continue;
                };
                if horse.2.contains(&interaction.data.custom_id) {
                    let _ = interaction.create_response(&ctx, CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new().content(format!("you already bet on {}",interaction.data.custom_id)).ephemeral(true))).await;
                    continue;
                }
                horse.2.push(interaction.data.custom_id.clone());
                // println!("awaiting discord edit {}: {} {}ms","interaction handler".to_string().blue(),format!("{} - {}",name,&horsename).black(),ins.elapsed().as_millis());
                drop(picks);
                println!("dropped picks in {}: {} {}ms","interaction handler".to_string().blue(),format!("{} - {}",name,&horsename).black(),ins.elapsed().as_millis());
                let _ = interaction.create_response(&ctx, CreateInteractionResponse::Acknowledge).await;
                // wagers_msg.edit(&ctx, EditMessage::new().embed(CreateEmbed::new()
                //             .title("wagers")
                //             .color(Color::DARK_GREEN)
                //             .field("", m, false))).await.unwrap();
                // println!("finished discord edit {}: {} {}ms","interaction handler".to_string().blue(),format!("{} - {}",name,&horsename).black(),ins.elapsed().as_millis());
            }
            println!("done responding to interactions");
        },
    );

    // wagers_msg.delete(&ctx).await?;
    let m = horses.iter().map(|(&name, _)| format!("- **{}**:\n🐴––––––––––––––––––––––––––––––|",name)).collect::<Vec<String>>().join("\n");
    let embed = CreateEmbed::new()
        .title("horse racing")
        .color(Color::DARK_ORANGE)
        .field("", "––––––––––––––––––––––––––––––––––––––––––––––––––––––––––––––––––––", false)
        .field("", m, false);
    message.edit(&ctx, EditMessage::new().components(vec![]).embed(embed)).await?;

    let mut games = ctx.data().active_games.lock().await;
    let horseracing = games.get_mut(&ctx.guild_id().unwrap()).unwrap().horse_racing.as_mut().unwrap();
    let picks = picks_mutex.lock().await;
    horseracing.betting = None;
    horseracing.wager_input = None;
    horseracing.pool = pool;
    horseracing.playercount = picks.len();
    for (_, w) in picks.iter() {
        for (i,v) in w.2.iter().enumerate() {
            let bets = horseracing.bets.get_mut(v.as_str()).unwrap();
            bets[i].push((w.0.clone(),w.1));
        }
        // horseracing.wagers.insert(*u, (w.0.name.clone(),w.1));
    }
    if horseracing.playercount == 1 {
        ctx.say("**only one better! playing singleplayer game**").await?;
    }
    
    Ok(())
}

#[poise::command(prefix_command,category = "gambling")]
pub async fn horseracing(ctx:Context<'_>) -> Result<(), Error> {
    let result = AssertUnwindSafe(_horseracing(&ctx)).catch_unwind().await;
    match result {
        Err(_) => {
            ctx.say("‼️ horse racing thread panicked (unrecoverably errored)! trying to refund betters..").await?;
            let mut games = ctx.data().active_games.lock().await;
            let Some(gg) = games.get_mut(&ctx.guild_id().unwrap()) else {
                ctx.say("this server had no games list").await?;
                return Ok(());
            };
            let Some(horseracing) = &gg.horse_racing else {
                ctx.say("no active horse race found").await?;
                return Ok(());
            };
            if horseracing.playercount == 0 {
                ctx.say("no betters! :)").await?;
                return Ok(());
            }
            let db = match Connection::open("saul.db") {
                Ok(db) => db,
                Err(dberr) => {
                    ctx.say(format!("error opening database: {}",dberr)).await.unwrap();
                    return Ok(());
                }
            };
            for (_,v) in &horseracing.bets {
                for i in v {
                    if i.is_empty() {continue};
                    for u in i {
                        db.execute("UPDATE saul SET agarthereum = agarthereum + ?2 WHERE discord_id = ?1",[i64::from(u.0.id),u.1 as i64])?;
                    }
                }
            }
            ctx.say("attempted to refund everyone. check your accounts").await?;
            gg.horse_racing = None;
            Ok(())
        },
        Ok(ok) => return ok
    }
}
async fn _horseracing(ctx:&Context<'_>) -> Result<(), Error> {
    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("not in a server").await?;
        return Ok(());
    };

    let mut games = ctx.data().active_games.lock().await;
    if let Some(gg) = games.get_mut(&guild_id) {
        if gg.horse_racing.is_some() {
            ctx.say("a horse racing match is already running in this server").await?;
            return Ok(());
        } else {
            gg.horse_racing = Some(HorseRacingData::default());
        }
    } else {
        games.insert(guild_id, crate::games::Games{horse_racing:Some(HorseRacingData::default()), ..Default::default()});
    }

    drop(games);

    let horsenames = Vec::from(crate::HORSE_NAMES);
    let mut horses: BTreeMap<&str, usize> = horsenames.choose_multiple(&mut rand::rng(), 5/*rand::random_range(5..8)*/).map(|&i| (i, 0)).collect();
    let horses_string: String = horses.iter().map(|(&name, _)| format!("- **{}**:\n🐴GET READY",name)).collect::<Vec<String>>().join("\n");
    
    let embed = CreateEmbed::new()
        .title("horse racing")
        .color(Color::DARK_ORANGE)
        .field("", "––––––––––––––––––––––––––––––––––––––––––––––––––––––––––––––––––––", false)
        .field("", &horses_string, false);

    let mut msg = ctx.send(CreateReply {
        content: Some(".🏇🏇🏇🏇🏇🏇🏇🏇🏇🏇🏇🏇🏇🏇🏇🏇🏇🏇🏇🏇🏇🏇🏇.".to_string()),
        embeds: vec![embed],
        ..Default::default()
    }).await?.into_message().await?;

    _hedge_bets_horse(&ctx, &mut horses, &mut msg, horses_string).await?;

    sleep(Duration::from_secs(1)).await;

    let mut games = ctx.data().active_games.lock().await;
    let horseracing = games.get_mut(&ctx.guild_id().unwrap()).unwrap().horse_racing.as_mut().unwrap();
    let horse_bets = horseracing.bets.clone();
    drop(games);


    let horse_name_strings = {
        let mut h = BTreeMap::new();
        for (i,v) in horse_bets {
            let mut s = format!("**{}**: ",i);
            if !v[0].is_empty() {
                s += "🥇";
                s += (v[0].iter().map(|x|&x.0.name).join(", ")).as_str(); 
            }
            if !v[1].is_empty() {
                s += "🥈";
                s += (v[1].iter().map(|x|&x.0.name).join(", ")).as_str(); 
            }
            if !v[2].is_empty() {
                s += "🥉";
                s += (v[2].iter().map(|x|&x.0.name).join(", ")).as_str(); 
            }
            h.insert(i,s);
        }
        h
    };

    let mut winners: Vec<&str> = vec![];
    let mut won = false;
    let mut tie = false;
    let mut highest: usize = 0;
    loop {
        // horses = horses.iter().map(|(&name, &dist)| (name, dist+(rand::random_range(1..3)))).collect();
        let mut result_string = "".to_string(); //= horses.iter().map(|(&name, &dist)| format!("- {}\n{}🏇",name,(0..dist).map(|_|"-").collect())).collect::<Vec<String>>().join("\n");
        let mut win_cache:Option<Vec<&str>> = None;

        for (&name, score) in &mut horses {
            *score += rand::random_range(1..3);
            if *score >= 30 {
                won = true;
                if !winners.contains(&name) {
                    if win_cache.is_none() {win_cache = Some(vec![])}
                    win_cache.as_mut().unwrap().push(name);
                }
                if *score > highest {highest = *score; tie = false;println!("tie reset");}
                else if *score == highest {tie = true; println!("TIE ALARM");}
                let _ = write!(result_string,"- {}\n{}🐴    🎉\n",horse_name_strings.get(name).unwrap(), (0..*score).map(|_| "–").collect::<String>());
                continue;
            }
            let _ = write!(result_string,"- {}\n{}🐴{}|\n",horse_name_strings.get(name).unwrap(), (0..*score).map(|_| "–").collect::<String>(), (*score..29).map(|_| " ").collect::<String>());
        }
        let embed = CreateEmbed::new()
            .title("horse racing")
            .color(Color::DARK_ORANGE)
            .field("", if tie {"*––TIEBREAKER––TIEBREAKER––TIEBREAKER––TIEBREAKER––TIEBREAKER––*"} else {"––––––––––––––––––––––––––––––––––––––––––––––––––––––––––––––––––––"}, false)
            .field("", result_string, false);
        msg.edit(ctx, EditMessage::new().embed(embed)).await?;
        if won {
            if win_cache.is_some() {
                winners.append(&mut win_cache.unwrap());
            }
            winners.sort_unstable_by_key(|k| horses.get(k).unwrap());
            winners.reverse();
            println!("sorted winners: {}",winners.join(", "));
            if !tie /*winners.len() >= 1*/ {
                winners.truncate(1);
                break;
            }
        }
        sleep(Duration::from_secs(1)).await;
    }

    let mut games = ctx.data().active_games.lock().await;
    let gg = games.get_mut(&guild_id).unwrap();
    let horseracing= gg.horse_racing.as_mut().unwrap();

    ctx.say(format!("🐴 **{}** won!!! 🎉🎉🎉🎉🎉🎉",winners[0])).await?;
    // ctx.say(format!("winners in order: {}",winners.join(", "))).await?;

    match horseracing.playercount {
    0 => (),
    1 => { 'singleplayer: {
        let bets = horseracing.bets.get_mut(winners[0]);
        let betters = bets.unwrap();
        let db = Connection::open("saul.db")?;
        for (i,v) in betters.iter().enumerate() {
            if v.is_empty() {continue};
            let wager = (v[0].1) as i64;
            match i {
                0 => {
                    ctx.say(format!("**came in 1st place!** tripled your money: 🪙{}",wager * 3)).await?;
                    db.execute("UPDATE saul SET agarthereum = agarthereum + ?2 WHERE discord_id = ?1",[i64::from(v[0].0.id),wager * 3])?;
                    break 'singleplayer;
                },
                1 => {
                    ctx.say(format!("**came in 2nd place!** doubled your money: 🪙{}",v[0].1 * 2)).await?;
                    db.execute("UPDATE saul SET agarthereum = agarthereum + ?2 WHERE discord_id = ?1",[i64::from(v[0].0.id),wager * 2])?;
                    break 'singleplayer;
                },
                2 => {
                    ctx.say(format!("**came in 3rd place!** got half your money back: 🪙{}",v[0].1 / 2)).await?;
                    db.execute("UPDATE saul SET agarthereum = agarthereum + ?2 WHERE discord_id = ?1",[i64::from(v[0].0.id),wager / 2])?;
                    break 'singleplayer;
                },
                _ => unreachable!()
            }
        }
        ctx.say("**none of your horses won!** loser").await?;
    }},
    _ => {  
        let mut total_weight = 0;
        let mut weights: HashMap<serenity::UserId,f64> = Default::default();
        // for (i,v) in winners.iter().enumerate() {
        //     let mut bets = horseracing.bets.get_mut(v);
        //     let betters = &mut bets.as_mut().unwrap()[i];
        //     for (user, wager) in betters {
        //         let weight = (3-i) * *wager;
        //         let payout = weight * horseracing.pool;
        //         total += payout;
        //         payouts.insert(user.name.clone(),payout);
        //     }
        // }
        let bets = horseracing.bets.get_mut(winners[0]);
        let betters = bets.unwrap();
        let db = Connection::open("saul.db")?;
        for (i,v) in betters.iter().enumerate() {
            for (user, wager) in v {
                let weight = (3-i) * *wager;
                total_weight += weight;
                weights.insert(user.id,weight as f64);
            }
        }
        let mut total: f64 = 0.0;
        for (i,w) in weights {
            let payout = horseracing.pool as f64 * w / total_weight as f64;
            total += payout;
            let payout_rounded = payout.ceil() as i64;
            db.execute("UPDATE saul SET agarthereum = agarthereum + ?2 WHERE discord_id = ?1",[i64::from(i),payout_rounded])?;
            ctx.say(format!("paid {} 🪙{} (weight {})",i.to_user(ctx).await?.name,payout_rounded, w)).await?;
        }
        ctx.say(format!("initial pool: {} total payout: {} (total weight {})",horseracing.pool, total, total_weight)).await?;
    }}

    gg.horse_racing = None;

    Ok(())

}

#[derive(strum::Display, Copy, Clone, Debug, PartialEq, Eq)]
enum Suit {
    Spades,
    Clubs,
    Diamonds,
    Hearts
}

#[derive(Default, Debug)]
struct Deck {
    in_play: Vec<Card>
}
impl Deck {
    fn draw(&mut self) -> Card {
        let card = Card {
            number: rand::random_range(1..=13),
            suit: match rand::random_range(0..4) {
                0 => Suit::Spades,
                1 => Suit::Clubs,
                2 => Suit::Diamonds,
                3 => Suit::Hearts,
                _ => unreachable!()
            }

        };
        if !self.in_play.contains(&card) {
            self.in_play.push(card);
        }
        return card;
    }
}

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
struct Card {
    number: u8,
    suit: Suit
}
impl Card {
    fn num_text(&self) -> &'static str {
        match self.number {
            1 => "Ace", 2 => "2", 3 => "3", 4 => "4", 5 => "5",
            6 => "6", 7 => "7", 8 => "8", 9 => "9", 10 => "10",
            11 => "Jack",
            12 => "Queen",
            13 => "King",
            _ => unreachable!()
        }
    }
    fn determine_value(&self, ace_as_11: bool) -> u8 {
        match self.number {
            1 => if ace_as_11 {11} else {1},
            2..10 => self.number,
            10..=13 => 10,
            _ => unreachable!()
        }
    }
}

#[derive(Default)]
pub struct BlackjackData {
    pub active_games: HashMap<u64, mpsc::Sender<bool>>
}

#[poise::command(prefix_command,category = "gambling")]
pub async fn blackjack(ctx:Context<'_>, wager:u32) -> Result<(), Error> {
    fn ace_aware_sum(hand: &Vec<Card>) -> u8 {
        let mut sum = {
            let mut sum = 0;
            for i in hand {
                sum += i.determine_value(true);
            }
            sum
        };
        if sum > 21 {
            let mut sum2 = 0;
            for i in hand {
                sum2 += i.determine_value(false);
            }
            sum = sum2
        };
        return sum;
    }

    let id = ctx.msg.author.id.get();
    let guild_id = ctx.guild_id().unwrap();
    let mut games = ctx.data().active_games.lock().await;
    if let Some(gg) = games.get_mut(&guild_id) {
        if gg.blackjack.active_games.contains_key(&id) {
            ctx.say("you already have an active blackjack game").await?;
            return Ok(());
        } //else {
        //     gg.blackjack.insert(id, None);
        // }
    } else {
        games.insert(guild_id, Default::default());
        // games.blackjack.insert(id, None);
    }
    drop(games);

    let dbid = i64::from(ctx.author().id);
    let db = _set_db_account(&ctx, dbid).await?;
    let amount: i64 = db.query_row("select id, agarthereum from saul where discord_id = ?1",
            [dbid],
            |row| Ok(row.get(1)?))?;

    ctx.say("blackjack: win 2x your money or lose it all\ndealer will stand on soft 17s").await?;
    let mut deck = Deck::default();
    let mut dealer_cards = vec![
        deck.draw(),
        deck.draw(),
    ];
    let mut your_cards = vec![
        deck.draw(),
        deck.draw(),
    ];

    ctx.say(format!("\
dealer's cards: {}, [?]
your cards: {}, {}
hit or stand?
",dealer_cards[0].num_text(),
your_cards[0].num_text(),your_cards[1].num_text())).await?;

    let mut gamedata = ctx.data().active_games.lock().await;
    let mut blackjackdata = &mut gamedata.get_mut(&guild_id).unwrap().blackjack;
    let (send, mut rec) = mpsc::channel(100);
    blackjackdata.active_games.insert(id, send);
    drop(gamedata);

    enum HandState {
        Bust,
        Perfect21,
        Continue
    }
 
    loop {
        match timeout(Duration::from_secs(30), rec.recv()).await {
            Ok(Some(chose_hit)) => {
                if chose_hit {
                    your_cards.push(deck.draw());
                    match ace_aware_sum(&your_cards) {
                        22.. => {
                            ctx.say(format!("your cards: {} **\\*BUST\\***\nbetter luck next time", your_cards.iter().map(|x| x.num_text()).join(", "))).await?;
                            let mut gamedata = ctx.data().active_games.lock().await;
                            let mut blackjackdata = &mut gamedata.get_mut(&guild_id).unwrap().blackjack;
                            blackjackdata.active_games.remove_entry(&id);
                            return Ok(())
                        }
                        21 => break,
                        ..21 => {
                            ctx.say(format!("your cards: {}\nhit or stand?", your_cards.iter().map(|x| x.num_text()).join(", "))).await?;
                        }
                    }
                } else {
                    break
                }
            }
            Ok(None) => println!("blackjack channel closed ??"),
            Err(_) => {ctx.say("took too long to decide").await?; break},
        }
    }
    let your_cards_string = your_cards.iter().map(|x| x.num_text()).join(", ");
    let mut msg = ctx.say(format!("\
dealer's cards: {}
your cards: {your_cards_string}", dealer_cards.iter().map(|x| x.num_text()).join(", ")
    )).await?.into_message().await?;

    let mut busted = false;
    loop {
        let dealer_cards_string = dealer_cards.iter().map(|x| x.num_text()).join(", ");
        msg.edit(&ctx, EditMessage::new().content(format!("\
dealer's cards: {dealer_cards_string}
your cards: {your_cards_string}"))).await?;
        let sum_soft = {
            let mut sum = 0;
            for i in &mut dealer_cards {
                sum += i.determine_value(true);
            }
            sum
        };
        let sum_hard = |hand: &mut Vec<Card>| {
            let mut sum = 0;
            for i in hand {
                sum += i.determine_value(false);
            }
            sum
        };
        if sum_soft <= 17 || sum_hard(&mut dealer_cards) <= 17 {
            sleep(Duration::from_secs(2)).await;
            msg.edit(&ctx, EditMessage::new().content(format!("\
    dealer's cards: {dealer_cards_string} [*hit!*]
your cards: {your_cards_string}"))).await?;
            sleep(Duration::from_secs(2)).await;
            dealer_cards.push(deck.draw());
        } else {
            if ace_aware_sum(&dealer_cards) > 21 {
                msg.edit(&ctx, EditMessage::new().content(format!("\
        dealer's cards: {dealer_cards_string} **\\*BUST\\***
your cards: {your_cards_string}"))).await?;
                busted = true;
            }  
            break
        }
    }

    if busted {
        ctx.say(format!("dealer busted! you win 🪙{}", wager*2)).await?;
    } else {
        let (dealer_sum, your_sum) = (ace_aware_sum(&dealer_cards), ace_aware_sum(&your_cards));
        if dealer_sum > your_sum {
            ctx.say("dealer won! you get nothing").await?;
        } else if dealer_sum == your_sum {
            ctx.say("tie! you get your money back.").await?;
        } else if dealer_sum < your_sum {
            ctx.say(format!("you win! you get 🪙{}", wager*2)).await?;
        }
    }

    let mut gamedata = ctx.data().active_games.lock().await;
    let mut blackjackdata = &mut gamedata.get_mut(&guild_id).unwrap().blackjack;
    blackjackdata.active_games.remove_entry(&id);

    Ok(())
}

#[poise::command(prefix_command,category = "gambling")]
pub async fn coalmines(ctx:Context<'_>) -> Result<(), Error> {
    let id = i64::from(ctx.author().id);
    let db = _set_db_account(&ctx, id).await?;

    let amount: i64 = db.query_row("select id, agarthereum from saul where discord_id = ?1",
            [id],
            |row| Ok(row.get(1)?))?;
    
    if amount >= 200 {
        ctx.say("work in the coal mines when you are below 🪙200 to recover money").await?;
        return Ok(());
    }
    ctx.say("working in the coal mines ⛰️⛏️").await?;
    ctx.say("https://tenor.com/view/spongebob-ai-generated-blue-collar-blue-collar-anthem-miner-gif-3731267226880813052").await?;
    sleep(Duration::from_secs(3)).await;
    let pay = rand::random_range(30..120);
    ctx.say(format!("{} mined 🪙{} ⚒️",ctx.author().mention(),pay)).await?;
    db.execute("UPDATE saul SET agarthereum = agarthereum + ?2 WHERE discord_id = ?1",[id,pay])?;
    Ok(())
}

#[poise::command(prefix_command,category = "gambling")]
pub async fn chinesesweatshop(ctx:Context<'_>) -> Result<(), Error> {
    let id = i64::from(ctx.author().id);
    let db = _set_db_account(&ctx, id).await?;

    let amount: i64 = db.query_row("select id, agarthereum from saul where discord_id = ?1",
            [id],
            |row| Ok(row.get(1)?))?;
    
    if amount >= 200 {
        ctx.say("work in a chinese sweatshop when you are below 🪙200 to recover money").await?;
        return Ok(());
    }
    ctx.say("working in a sweatshop 🇨🇳👶").await?;
    ctx.say("https://cdn.discordapp.com/attachments/1000574112546168843/1444257508670570620/image0.jpg?ex=692c0d1f&is=692abb9f&hm=b64bb65a7b9edb9b52a6b707bfc93a8136d6d7cad3a44d139ee1eb35265424be&").await?;
    sleep(Duration::from_secs(3)).await;
    let pay = rand::random_range(30..120);
    match rand::random_range(0..2) {
        0 => {ctx.say(format!("{} produced prime paraphernalia for the polyester prince 🪙{}",ctx.author().mention(),pay)).await?;
            ctx.say("https://encrypted-tbn0.gstatic.com/images?q=tbn:ANd9GcQlphM7gLkOEYmFO6XFfX7FJTbUqO_CLkrZQqPAc6brOR8ZScgF").await?;},
        1 => {ctx.say(format!("{} made the whole set for granny the temu warrior 🪙{} ⚒️",ctx.author().mention(),pay)).await?;
            ctx.say("https://img.kwcdn.com/product/open/2024-06-24/1719191001020-a2058a65c05f449982bf08fd029b9124-goods.jpeg").await?;},
        2 => {ctx.say(format!("{} slaved away in the shein factory 🪙{} ⚒️",ctx.author().mention(),pay)).await?;
            ctx.say("https://external-preview.redd.it/fbre7qrvjmkk6cg5stRdk2xUcuUBXJnuAfVptAgWiaQ.png?format=pjpg&auto=webp&s=d99b89ec45109a9ac60ddcb3b659ea5d51221935").await?;}
        _ => unreachable!()
    }
    
    db.execute("UPDATE saul SET agarthereum = agarthereum + ?2 WHERE discord_id = ?1",[id,pay])?;
    Ok(())
    // 
}
