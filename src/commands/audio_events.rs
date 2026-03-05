use colored::Colorize;
use lavalink_rs::{hook, model::events, prelude::*};
// use poise::serenity_prelude::{ChannelId, Http};

#[hook]
pub async fn raw_event(_: LavalinkClient, session_id: String, event: &serde_json::Value) {
    if event["op"].as_str() == Some("event") || event["op"].as_str() == Some("playerUpdate") {
        println!("{:?} -> {:?}", session_id, event);
    }
}

#[hook]
pub async fn ready_event(client: LavalinkClient, session_id: String, event: &events::Ready) {
    let _ = client.delete_all_player_contexts().await;
    println!("{}: {:?} -> {:?}", "Ready".bright_green(), session_id, event);
}

#[hook]
pub async fn track_start(_client: LavalinkClient, _session_id: String, event: &events::TrackStart) {
    // let player_context = client.get_player_context(event.guild_id).unwrap();
    // let data = player_context.data::<(ChannelId, std::sync::Arc<Http>)>().unwrap();
    // let (channel_id, http) = (&data.0, &data.1);

    let msg = {
        let track = &event.track;

        format!("playing '{}'",track.info.title.bright_purple())
    };

    println!("{}",msg);
}