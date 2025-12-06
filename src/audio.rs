use std::time::Duration;

use lavalink_rs::{model::player as lavaplayer, prelude::*};

use poise::{CreateReply, serenity_prelude::{self as serenity, Color, CreateEmbed, futures::StreamExt}};

use crate::{Context,Error};

async fn _join(ctx: &Context<'_>, guild_id: serenity::GuildId, channel_id: Option<serenity::ChannelId>) -> Result<(), Error> {
    let lavaclient = ctx.data().lavalink.clone();

    let manager = songbird::get(ctx.serenity_context()).await.expect("songbird client placed at init").clone();

    if lavaclient.get_player_context(guild_id).is_none() {
        let connect_to = match channel_id {
            Some(x) => x,
            None => {
                let user_chan = ctx.guild().unwrap().voice_states.get(&ctx.author().id).and_then(|voice_state|voice_state.channel_id);
                match user_chan {
                    Some(chan) => chan,
                    None => {
                        ctx.say("you're not connected to a voice chat").await?;

                        return Err("not in voice channel".into());
                    }
                }
            }
        };

        let handler = manager.join_gateway(guild_id, connect_to).await; 

        match handler {
            Ok((connection_info, _)) => {
                let info = lavaplayer::ConnectionInfo {
                    token: connection_info.token,
                    endpoint: connection_info.endpoint,
                    session_id: connection_info.session_id,
                };
                if let Err(err) = lavaclient.create_player_context(guild_id, info).await {
                    match err {
                        lavalink_rs::error::LavalinkError::HyperClientError(h) => {
                            ctx.say("audio server isn't running 😿").await?;
                            return Err(h.into());
                        },
                        _ => {
                            ctx.say(format!("😿 error creating audio server connection: `{}`",err)).await?;
                            return Err(err.into());
                        }
                    }
                };
            },
            Err(why) => {
                ctx.say(format!("😿 error joining the channel: {}",why)).await?;
                return Err(why.into());
            }
        }
    };

    Ok(())
}

/// play a song
#[poise::command(prefix_command,category = "audio",aliases("p"))]
pub async fn play(ctx:Context<'_>, #[rest] query:Option<String>) -> Result<(),Error> {

    let Some(query) = query else {
        ctx.say("no song provided").await?;
        return Ok(());
    };

    let guild_id = ctx.guild_id().unwrap();

    let lavaclient = ctx.data().lavalink.clone();

    let player = match lavaclient.get_player_context(guild_id) {
        Some(p) => p,
        None => {
            if let Err(_) = _join(&ctx, guild_id, None).await {
                return Ok(());
            };
            lavaclient.get_player_context(guild_id).expect("couldn't find lava context even after trying to join")
        }
    };

    let term = if query.starts_with("http://") | query.starts_with("https://") {
        query
    } else {
        match SearchEngines::YouTube.to_query(&query) {
            Ok(t) => t,
            Err(err) => {
                ctx.say(format!("**ran into an error 😿** ```{}```",err)).await?;
                return Err(err.into());
            }
        }
    };

    let loaded_tracks = lavaclient.load_tracks(guild_id, &term).await?;

    // let mut playlist_info = None;

    let tracks: Vec<TrackInQueue> = match loaded_tracks.data {
        Some(TrackLoadData::Track(x)) => vec![x.into()],
        Some(TrackLoadData::Search(x)) => vec![x[0].clone().into()],
        Some(TrackLoadData::Playlist(_)) => {
            // playlist_info = Some(x.info);
            // x.tracks.iter().map(|x| x.clone().into()).collect()
            ctx.say("playlists aren't supported 😿").await?;
            return Ok(());
        },
        None => {
            ctx.say(format!("no results found with that search 😿")).await?;
            return Ok(());
        }
        _ => {
            ctx.say(format!("track error: {:?}", loaded_tracks)).await?;
            return Ok(());
        }
    };

    let queue = player.get_queue();
    let track = &tracks[0].track;

    let count = queue.get_count().await?;
    let title_string = match &track.info.uri {
        Some(t) => format!("[**{}**]({})",track.info.title,t),
        None => format!("**{}**",track.info.title)
    };
    
    if player.get_player().await?.track.is_some() {
        ctx.say(format!("#{} - '{}' added to queue", count+1, title_string)).await?;
        queue.append(tracks.into())?;
    } else {
        ctx.say(format!("now playing: '{}'",title_string)).await?;
        if let Err(err) = player.play(track).await {
            ctx.say(format!("😿 error playing song: {}",err)).await?;
            return Err(err.into());
        }
    }

    // for i in &mut tracks {
    //     i.track.user_data = Some(serde_json::json!({"requester_id": ctx.author().id.get()}));
    // }

    Ok(())
}

/// stop playing and disconnect saul
#[poise::command(prefix_command,category = "audio")]
pub async fn stop(ctx:Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let manager = songbird::get(ctx.serenity_context()).await.unwrap().clone();
    let lavaclient = ctx.data().lavalink.clone();
    
    lavaclient.delete_player(guild_id).await?;

    if manager.get(guild_id).is_some() {
        manager.remove(guild_id).await?;
    }
    
    Ok(())
}

/// clear the queue and stop all songs
#[poise::command(prefix_command,category = "audio")]
pub async fn clear(ctx:Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let lavaclient = ctx.data().lavalink.clone();

    let Some(player) = lavaclient.get_player_context(guild_id) else {
        ctx.say("not in a voice channel").await?;
        return Ok(());
    };

    let _ = player.get_queue().clear();

    ctx.say("cleared queue").await?;

    Ok(())
}

/// skip the currently playing song
#[poise::command(prefix_command,category = "audio")]
pub async fn skip(ctx:Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let lavaclient = ctx.data().lavalink.clone();

    let Some(player) = lavaclient.get_player_context(guild_id) else {
        ctx.say("not in a voice channel").await?;
        return Ok(());
    };

    let now_playing = player.get_player().await?.track;

    if let Some(np) = now_playing {
        ctx.say(format!("skipped '**{}**'",np.info.title)).await?;
        player.skip()?;
    }

    Ok(())
}

/// view the current queue
#[poise::command(prefix_command,category = "audio",aliases("q"))]
pub async fn queue(ctx:Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let lavaclient = ctx.data().lavalink.clone();

    let Some(player) = lavaclient.get_player_context(guild_id) else {
        ctx.say("not in a voice channel").await?;
        return Ok(());
    };

    let now_playing = player.get_player().await?.track;

    let queue = player.get_queue();

    let response = queue
        .enumerate()
        .map(|(id, t)| {
            format!("#{} - '**{}**'",id+1,t.track.info.title)
        })
        .collect::<Vec<String>>()
        .await
        .join("\n");

    // ctx.say(format!("▶ currently playing: {}\nin queue:\n{}",
    //     match now_playing {Some(track) => format!("'**{}**'",track.info.title), None => "nothing :)".to_string()},
    //     if response.is_empty() {"nothing :)"} else {&response}
    //     )).await?;

    let embed = match now_playing {
        Some(track) => {
            let position = player.get_player().await?.state.position;
            let progress = (25*position).div_ceil(track.info.length);
            let pgfilled = (0..progress).map(|_| "▰").collect::<String>();
            let pgempty = (0..25-progress).map(|_| "▱").collect::<String>();

            CreateEmbed::new()
                .title(format!("NOW PLAYING: {}",track.info.title)).url(track.info.uri.unwrap_or_default())
                .thumbnail(format!("https://img.youtube.com/vi/{}/default.jpg",track.info.identifier))
                .field("", format!("**author:** {}", track.info.author), false)
                .field("", format!("{} / {}", format_timestamp(position), format_timestamp(track.info.length)), false)
                .field("", format!("{}", format!("[{} 🔘 {}]",pgfilled,pgempty)), false)
                .color(Color::RED)
                .field("in queue", if response.is_empty() {"nothing :)"} else {&response}, false)
        },
        None => {
            CreateEmbed::new()
                .title("NOW PLAYING: nothing")
                .thumbnail("https://pbs.twimg.com/media/B4NEmVVIMAE3DQS.png")
                .field("in queue", if response.is_empty() {"nothing :)"} else {&response}, false)
        }
    };
    ctx.send(CreateReply {
            embeds: vec![embed],
            ..Default::default()
        }).await?;
    Ok(())
}

fn format_timestamp(time: u64) -> String {
    let sec = (time/1000)%60;
    let min = (time/60000)%60;
    let hour = time/3600000;

    return if hour > 0 {
        format!("{:02}:{:02}:{:02}",hour,min,sec)
    } else {
        format!("{:02}:{:02}",min,sec)
    }
}

#[poise::command(prefix_command,category = "audio",aliases("np"))]
pub async fn nowplaying(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let lavaclient = ctx.data().lavalink.clone();

    let Some(player) = lavaclient.get_player_context(guild_id) else {
        ctx.say("not in a voice channel").await?;
        return Ok(());
    };

    let now_playing = player.get_player().await?.track;
    let embed = match now_playing {
        Some(track) => {
            let position = player.get_player().await?.state.position;
            let progress = (25*position).div_ceil(track.info.length);
            let pgfilled = (0..progress).map(|_| "▰").collect::<String>();
            let pgempty = (0..25-progress).map(|_| "▱").collect::<String>();

            CreateEmbed::new()
                .title(format!("NOW PLAYING: {}",track.info.title)).url(track.info.uri.unwrap_or_default())
                .thumbnail(format!("https://img.youtube.com/vi/{}/default.jpg",track.info.identifier))
                .field("", format!("**author:** {}", track.info.author), false)
                .field("", format!("{} / {}", format_timestamp(position), format_timestamp(track.info.length)), false)
                .field("", format!("{}", format!("[{} 🔘 {}]",pgfilled,pgempty)), false)
                .color(Color::RED)
        },
        None => CreateEmbed::new().title("NOW PLAYING: nothing").thumbnail("https://pbs.twimg.com/media/B4NEmVVIMAE3DQS.png")
    };
    ctx.send(CreateReply {
            embeds: vec![embed],
            ..Default::default()
        }).await?;
    Ok(())
}


/// skip to a timestamp in the song
#[poise::command(prefix_command,category = "audio",aliases("seek","timestamp"))]
pub async fn skipto(ctx: Context<'_>, timestamp:Option<String>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let lavaclient = ctx.data().lavalink.clone();

    let Some(player) = lavaclient.get_player_context(guild_id) else {
        ctx.say("not in a voice channel").await?;
        return Ok(());
    };

    let Some(timestamp) = timestamp else {
        ctx.say("no timestamp provided").await?;
        return Ok(());
    };

    // let Some(now_playing) = player.get_player().await?.track else {
    //     ctx.say("no song currently playing").await?;
    //     return Ok(());
    // };

    let time = Duration::from_secs( if timestamp.contains(":") {
        let split = timestamp.split(":").collect::<Vec<&str>>();
        if split.len() > 3 || split.len() < 1 {
            ctx.say("invalid timestamp").await?;
            return Ok(());
        }

        let mut nums:Vec<u64> = vec![];
        for i in split {
            match i.parse::<u64>() {
                Ok(n) => {nums.push(n);},
                Err(_) => {
                    ctx.say("invalid timestamp").await?;
                    return Ok(());
                }
            }
        }
        match nums.len() {
            3 => 3600*nums[0]+60*nums[1]+nums[2],
            2 => 60*nums[0]+nums[1],
            1 => nums[0],
            _ => unreachable!()
        }
    } else {
        match timestamp.parse::<u64>() {
            Ok(t) => t,
            Err(_) =>{
                ctx.say("invalid timestamp").await?;
                return Ok(()); 
            }
        }
    });

    match player.set_position(time).await {
        Ok(_) => {
            ctx.say("skipped to timestamp").await?;
        },
        Err(err) => {
            ctx.say(format!("😿 ran into an error: {}",err)).await?;
            return Err(err.into());
        }
    }

    Ok(())
}

/// rewind seconds (default 5)
#[poise::command(prefix_command,category = "audio",aliases("rw"))]
pub async fn rewind(ctx: Context<'_>, time:Option<String>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let lavaclient = ctx.data().lavalink.clone();

    let Some(player) = lavaclient.get_player_context(guild_id) else {
        ctx.say("not in a voice channel").await?;
        return Ok(());
    };

    let pos = Duration::from_millis(player.get_player().await?.state.position);
    
    let skipamnt = if let Some(t) = time {
        let num = match t.parse::<u64>() {
            Ok(n) => n,
            Err(_) => {
                ctx.say("invalid time").await?;
                return Ok(());
            }
        };

        Duration::from_secs(num)
    } else {
        Duration::from_secs(5)
    };

    let rewind_time = if pos > skipamnt {
        pos - skipamnt
    } else {
        Duration::ZERO
    };

    match player.set_position(rewind_time).await {
        Ok(_) => {
            ctx.say(format!("rewund {} seconds", skipamnt.as_secs())).await?;
        },
        Err(err) => {
            ctx.say(format!("😿 ran into an error: {}",err)).await?;
            return Err(err.into());
        }
    }

    Ok(())
}

/// fast forward seconds (default 5)
#[poise::command(prefix_command,category = "audio",aliases("forward","fw"))]
pub async fn fastforward(ctx: Context<'_>, time:Option<String>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let lavaclient = ctx.data().lavalink.clone();

    let Some(player) = lavaclient.get_player_context(guild_id) else {
        ctx.say("not in a voice channel").await?;
        return Ok(());
    };

    let pos = Duration::from_millis(player.get_player().await?.state.position);
    
    let skipamnt = if let Some(t) = time {
        let num = match t.parse::<u64>() {
            Ok(n) => n,
            Err(_) => {
                ctx.say("invalid time").await?;
                return Ok(());
            }
        };

        Duration::from_secs(num)
    } else {
        Duration::from_secs(5)
    };

    let fwd_time = pos + skipamnt;

    match player.set_position(fwd_time).await {
        Ok(_) => {
            ctx.say(format!("fast forwarded {} seconds", skipamnt.as_secs())).await?;
        },
        Err(err) => {
            ctx.say(format!("😿 ran into an error: {}",err)).await?;
            return Err(err.into());
        }
    }

    Ok(())
}

/// pause
#[poise::command(prefix_command,category = "audio")]
pub async fn pause(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let lavaclient = ctx.data().lavalink.clone();

    let Some(player) = lavaclient.get_player_context(guild_id) else {
        ctx.say("not in a voice channel").await?;
        return Ok(());
    };

    player.set_pause(true).await?;
    ctx.say("paused ⏸️").await?;

    Ok(())
}

/// resume
#[poise::command(prefix_command,category = "audio")]
pub async fn resume(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let lavaclient = ctx.data().lavalink.clone();

    let Some(player) = lavaclient.get_player_context(guild_id) else {
        ctx.say("not in a voice channel").await?;
        return Ok(());
    };

    player.set_pause(false).await?;
    ctx.say("resumed ▶️").await?;

    Ok(())
}

/// sets playback setting to the specified setting
#[poise::command(prefix_command,category="audio")]
pub async fn set(ctx: Context<'_>, setting: String, parameter: f64) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let lavaclient = ctx.data().lavalink.clone();

    let Some(player) = lavaclient.get_player_context(guild_id) else {
        ctx.say("not in a voice channel").await?;
        return Ok(());
    };

    let existing_filter = player.get_player().await?.filters.expect("no existing filters");

    let mut timescale = existing_filter.timescale.unwrap_or_default();

    match setting.as_str() {
        "speed" => timescale.speed = Some(parameter),
        "pitch" => timescale.pitch = Some(parameter),
        "rate" => timescale.rate = Some(parameter),
        _ => {
            ctx.say("invalid playback setting").await?;
            return Ok(());
        }
    }

    player.set_filters(lavaplayer::Filters {
        timescale: Some(timescale),
        ..existing_filter
    }).await?;

    ctx.say(format!("set {} to {}",setting, parameter)).await?;

    Ok(())
}

#[poise::command(prefix_command,hide_in_help,category = "audio")]
pub async fn joe(ctx:Context<'_>, #[rest] biden: Option<String>) -> Result<(), Error> {
    let Some(biden) = biden else {
        return Ok(());
    };
    if !biden.starts_with("biden mode") {return Ok(());}

    let guild_id = ctx.guild_id().unwrap();
    let lavaclient = ctx.data().lavalink.clone();

    let Some(player) = lavaclient.get_player_context(guild_id) else {
        return Ok(());
    };

    let existing_filter = player.get_player().await?.filters.expect("no existing filters");

    if biden == "biden mode activate" {
        player.set_filters(lavaplayer::Filters {
            vibrato: Some(lavaplayer::TremoloVibrato {
                frequency: Some(14.0),
                depth: Some(1.0)
            }),
            ..existing_filter
        }).await?;
    }
    if biden == "biden mode deactivate" {
        player.set_filters(lavaplayer::Filters::default()).await?;
    }
    Ok(())
}