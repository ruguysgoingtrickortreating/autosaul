use std::sync::Arc;

use poise::serenity_prelude::futures::StreamExt;
use poise::serenity_prelude::{async_trait, EditMessage};
use songbird::error::JoinError;
use songbird::events::{Event, EventContext, EventHandler as VoiceEventHandler};
use songbird::input::Compose;
use songbird::tracks::Track;
use songbird::{input::YoutubeDl};

use colored::Colorize;

use crate::{Context,Error};

async fn join_vc(guild_id: poise::serenity_prelude::GuildId, channel_id: poise::serenity_prelude::ChannelId, manager: Arc<songbird::Songbird>) -> Result<(), JoinError> {

    // match mgr.join(ctx.guild_id().into(), channel).await {
    //     Ok(handler_lock) => {
    //         let mut handler = handler_lock.lock().await;
    //         handler.add_global_event(songbird::TrackEvent::Error.into(), TrackErrNotifier);
    //     },
    //     Err(err) => return err
    // }

    let handler_lock = manager.join(guild_id,channel_id).await?;
    let mut handler = handler_lock.lock().await;
    handler.add_global_event(songbird::TrackEvent::Error.into(), TrackErrNotifier);

    Ok(())
}

struct TrackErrNotifier;
#[async_trait]
impl VoiceEventHandler for TrackErrNotifier {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(track_list) = ctx {
            for (state, handle) in *track_list {
                println!("{}",format!("⚠️ Track {:?} encountered an error: {:?}",handle.uuid(),state.playing).red());
            }
        }

        None
    }
}

/// play a song
#[poise::command(prefix_command,category = "audio",aliases("p"))]
pub async fn play(ctx:Context<'_>, #[rest] query:Option<String>) -> Result<(),Error> {

    let mgr = songbird::get(ctx.serenity_context()).await.expect("songbird client placed at init").clone();

    if let None = mgr.get(ctx.guild_id().unwrap()) {
        let chan_id = ctx.guild().unwrap().voice_states.get(&ctx.author().id).and_then(|voice_state|voice_state.channel_id);
        match chan_id {
            None => {
                ctx.say("you're not connected to a voice chat").await?;
                return Ok(())
            },
            Some(id) => {
                if let Err(err) = join_vc(ctx.guild_id().unwrap(),id, mgr.clone()).await {
                    ctx.say(format!("couldn't join channel: {}",err)).await?;
                    return Ok(())
                }
            },
        };
    }
    
    if let None = query {
        ctx.say("no song provided").await?;
        return Ok(())
    }
    let query = query.unwrap();

    let mut search_msg = ctx.channel_id().say(ctx.http(),"completing that last command...").await?;

    let as_url = query.starts_with("http://") | query.starts_with("https://");

    let http_client = ctx.data().http_client.clone();

    if let Some(handler_lock) = mgr.get(ctx.guild_id().unwrap()) {
        let mut handler = handler_lock.lock().await;

        let mut src = if as_url {
            YoutubeDl::new(http_client, query)
        } else {
            YoutubeDl::new_search(http_client, query)
        };

        search_msg.edit(ctx.http(), EditMessage::new().content("searching...")).await?;

        let metadata = match src.aux_metadata().await {
            Ok(data) => data,
            Err(err) => {
                let mut reply = ctx.channel_id().say(ctx.http(),format!("**ran into an error 😿** ```{}```",err)).await?;
                let message_updates = poise::serenity_prelude::collector::collect(&ctx.serenity_context().shard, move |ev| match ev {
                    poise::serenity_prelude::Event::MessageUpdate(x) if x.id == reply.id => Some(()),
                    _ => None
                });
                let _ = tokio::time::timeout(std::time::Duration::from_millis(2000), message_updates.enumerate().next()).await;
                reply.edit(&ctx.http(), EditMessage::new().suppress_embeds(true)).await?;
                return Ok(())
            }
        };

        let src_name = metadata.title.unwrap_or("[no title]".to_string());

        let track_num = handler.queue().current_queue().len() + 1;
        
        search_msg.edit(ctx.http(), EditMessage::new().content(format!("#{} - '**{}**' added to queue",track_num,src_name))).await?;

        let track = Track::new_with_data(src.clone().into(), Arc::new(src_name));

        let _ = handler.enqueue(track).await;
    } else {
        ctx.say("failed to join 😿").await?;
    }

    Ok(())
}


#[poise::command(prefix_command,category = "audio")]
pub async fn stop(ctx:Context<'_>) -> Result<(), Error> {
    let mgr = songbird::get(ctx.serenity_context()).await.expect("songbird client at init").clone();

    let has_handler = mgr.get(ctx.guild_id().unwrap()).is_some();

    if has_handler {
        if let Err(e) = mgr.remove(ctx.guild_id().unwrap()).await {
            ctx.say(format!("error: {:?}",e)).await?;
        };
    }

    Ok(())
}

#[poise::command(prefix_command,category = "audio")]
pub async fn clear(ctx:Context<'_>) -> Result<(), Error> {
    let mgr = songbird::get(ctx.serenity_context()).await.expect("songbird client at init").clone();

    if let Some(handler_lock) = mgr.get(ctx.guild_id().unwrap()) {
        let handler = handler_lock.lock().await;
        
        handler.queue().stop();

        ctx.say("cleared queue").await?;
    } else {
        ctx.say("not in a voice channel").await?;
    }

    Ok(())
}

#[poise::command(prefix_command,category = "audio")]
pub async fn skip(ctx:Context<'_>) -> Result<(), Error> {
    let mgr = songbird::get(ctx.serenity_context()).await.expect("songbird client at init").clone();

    if let Some(handler_lock) = mgr.get(ctx.guild_id().unwrap()) {
        let handle = handler_lock.lock().await;
        if let Err(err) = handle.queue().skip() {
            ctx.say(format!("ran into an error: {:?}",err)).await?;
        } // else {
        //     ctx.say("skipped").await?;
        // }
    } else {
        ctx.say("not in a voice channel").await?;
    }

    Ok(())
}

#[poise::command(prefix_command,category = "audio",aliases("q"))]
pub async fn queue(ctx:Context<'_>) -> Result<(), Error> {
    let mgr = songbird::get(ctx.serenity_context()).await.expect("songbird client at init").clone();

    if let Some(handler_lock) = mgr.get(ctx.guild_id().unwrap()) {
        let handle = handler_lock.lock().await;
        let queue = handle.queue();

        let mut response = String::new();

        for (i,v) in queue.current_queue().iter().skip(1).enumerate() {
            response.push_str(&format!("#{} - '**{}**'\n",i+1,(&v.data::<String>())));
        }

         ctx.say(format!("▶ currently playing: '**{}**'\nin queue:\n{}",
            match queue.current(){Some(handle) => handle.data::<String>(), None => Arc::new("no currently playing song".to_string())},
            if response.is_empty() {"nothing :)"} else {&response}
            )).await?;
    } else {
        ctx.say("not in a voice channel").await?;
    }

    Ok(())
}