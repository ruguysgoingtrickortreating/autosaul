use poise::{CreateReply, serenity_prelude::{Color, CreateActionRow, CreateButton, CreateEmbed, CreateInteractionResponse, CreateInteractionResponseMessage, EditMessage, MessageId, UserId, futures::StreamExt}};
use std::{collections::BTreeMap, process, time::Duration};
use itertools::Itertools;
use poise::serenity_prelude::{ComponentInteractionCollector, GetMessages};
use crate::{Context, Error};

/// show the help menu
#[poise::command(prefix_command)]
pub async fn help(ctx:poise::Context<'_,crate::Data,Error>, #[description="specific command to show help about"] command: Option<String>) -> Result<(),Error> {
    poise::builtins::help(ctx, command.as_deref(), 
        poise::builtins::HelpConfiguration {
            extra_text_at_bottom: "copyright duckroll conglomerate 2013",
            ..Default::default()
        }
    ).await?;
    Ok(())
}

/// saul
#[poise::command(prefix_command)]
pub async fn saul(ctx:Context<'_>) -> Result<(), Error> {
    ctx.say("https://tenor.com/view/3d-saul-saul-goodman-adamghik-gif-23876766").await?;
    Ok(())
}

#[poise::command(prefix_command, hide_in_help, aliases("r"))]
pub async fn restart(ctx:Context<'_>) -> Result<(),Error> {
    if ctx.author().id != 640722508093325342 {
        ctx.say("you don't have permission to restart the bot").await?;
        return Ok(())
    }
    ctx.say("restarting...").await?;
    process::exit(0)
}

/// get a message from the current channel
#[poise::command(prefix_command)]
pub async fn getmessage(ctx:Context<'_>, #[description="id of the message"] message_id: Option<String>) -> Result<(), Error> {
    match message_id {
        Some(msg_id) => {
            let msg_id = msg_id.parse::<u64>();
            match msg_id {
                Ok(id) => {
                    let msg_id_type = MessageId::new(id);
                    let msg = ctx.http().get_message(ctx.channel_id(), msg_id_type).await;
                    match msg {
                        Ok(msg) => ctx.say(format!("{} said: {}",msg.author.name,msg.content)).await?,
                        Err(err) => ctx.say(format!("was unable to get message: {}",err)).await?,
                    }
                },
                Err(_) => ctx.say("invalid id").await?
            }
        },
        None => ctx.say("no message id provided").await?,
    };
    Ok(())
}

/// ban a user forever and automatically report their account to discord and the ICE hotline
#[poise::command(prefix_command)]
pub async fn ban(ctx:poise::PrefixContext<'_, crate::Data, Error>, #[description="target of ban"] target: Option<String>, #[description="reason for ban"] #[rest] rest_of_message: Option<String>) -> Result<(), Error> {
    if ctx.msg.content.to_lowercase().contains("carter pewterschmidt") {
        let embed = CreateEmbed::new()
            .title(format!("❌ Carter Pewterschmidt was banned"))
            .color(Color::RED)
            .thumbnail("https://media.licdn.com/dms/image/v2/D4D03AQHhcVq741M3Og/profile-displayphoto-shrink_200_200/profile-displayphoto-shrink_200_200/0/1716171237526?e=2147483647&v=beta&t=Q1YtnBId2MwH8LfbKdjkVdQ-oT9mGg3eckzp4i_kROA")
            .field(format!("Carter Pewterschmidt was banned with reason:"), format!("{}",rest_of_message.unwrap_or("[no reason provided]".to_string())), true);

        ctx.send(CreateReply {
            embeds: vec![embed],
            ..Default::default()
        }).await?;
        return Ok(())
    }

    match target {
        Some(target) => {
            let usr_id = if target.contains("<@") {
                target[2..target.len()-1].parse::<u64>()
            } else {
                target.parse::<u64>()
            };
            match usr_id {
                Ok(usr_id) => {
                    let usr_id_type = UserId::new(usr_id);
                    let user = usr_id_type.to_user(ctx.http()).await;
                    match user {
                        Ok(user) => {
                            let embed = CreateEmbed::new()
                                .title(format!("❌ User {} was banned",user.name))
                                .color(Color::RED)
                                .thumbnail(user.static_face())
                                .field(format!("Member {} was banned with reason:",user.name), format!("{}",rest_of_message.unwrap_or("[no reason provided]".to_string())), true);

                            ctx.send(CreateReply {
                                embeds: vec![embed],
                                ..Default::default()
                            }).await?
                        },
                        
                        Err(err) => ctx.say(format!("error getting user: {}",err)).await?
                    }
                }
                Err(_) => ctx.say("invalid id").await?
            }
        },
        None => ctx.say("must provide a user to ban").await?
    };
    Ok(())
}

#[poise::command(prefix_command)]
pub async fn deletesince(ctx:Context<'_>, msg_id: u64, amount:u8) -> Result<(), Error> {
    let Some(guild_id) = ctx.guild_id() else {
        return Ok(());
    };
    if ctx.author().id != 640722508093325342 {
        ctx.say("not authorized to do that").await?;
        return Ok(());
    }
    if amount > 100 {
        ctx.say("can only delete up to 100 messages").await?;
        return Ok(())
    }

    let messages = ctx.channel_id().messages(&ctx,
        GetMessages::new().after(MessageId::from(msg_id)).limit(amount)
    ).await?;

    let count = messages.len();
    if messages.is_empty() {
        ctx.say("this would not delete any messages.").await?;
        return Ok(());
    }
    if count < 2 {
        ctx.say("this would not delete enough messages (min 2).").await?;
        return Ok(());
    }

    let msg = ctx.send(CreateReply::default()
        .content(format!("this would delete {} messages, starting with\
            https://discord.com/channels/{}/{}/{} and ending with\
            https://discord.com/channels/{1}/{2}/{4}. are you sure?",
            count,
            guild_id, ctx.channel_id(), messages[count-1].id, messages[0].id
        ))
        .components(vec![CreateActionRow::Buttons(vec![
            CreateButton::new("yes").label("yes"),
            CreateButton::new("no").label("no")
        ])])
    ).await?.into_message().await?;

    if let Some(interaction) = ComponentInteractionCollector::new(&ctx)
        .message_id(msg.id)
        .author_id(ctx.author().id)
        .timeout(Duration::from_secs(60))
        .await
    {
        msg.delete(&ctx).await?;
        if interaction.data.custom_id == "yes" {
            ctx.channel_id().delete_messages(&ctx,
                messages.iter().map(|m| m.id)
            ).await?;
            ctx.say(format!("deleted {count} messages")).await?;
        } else if interaction.data.custom_id == "no" {
        }
    } else {
        msg.delete(&ctx).await?;
        ctx.say("took too long to choose").await?;
    }

    Ok(())
}

/// here comes the tickle monster
#[poise::command(prefix_command)]
pub async fn tickle(ctx:Context<'_>) -> Result<(),Error> {
    let mut laugh = String::new();
    let mut laughed_once = false;
    loop {
        match rand::random_range(0..7) {
            0..3 => {laugh.push_str("hee"); laughed_once = true},
            3..6 => {laugh.push_str("ha"); laughed_once = true},
            6 => if laughed_once {break},
            _ => unreachable!()
        }
    }
    ctx.say(laugh).await?;
    Ok(())
}

#[poise::command(prefix_command,category = "gambling")]
pub async fn streamtest(ctx:Context<'_>) -> Result<(), Error> {
    let mut msg = ctx.send(CreateReply {
        content: Some("diddly".to_string()),
        components: Some(vec![CreateActionRow::Buttons(vec![
            CreateButton::new("goon shoe").label("goon shoe"),
            CreateButton::new("goon balenciaga").label("goon balenciaga"),
            CreateButton::new("goon vlone").label("goon vlone")])]),
        ..Default::default()
    }).await?.into_message().await?;

    let mut interaction_stream = msg.await_component_interaction(&ctx.serenity_context().shard).timeout(Duration::from_mins(1)).stream();
    let mut users: BTreeMap<String, Vec<String>> = Default::default();

    while let Some(interaction) = interaction_stream.next().await {
        let id = interaction.data.custom_id.clone();
        if let Some(v) = users.get_mut(&interaction.user.name) {
            if !v.contains(&id) {
                v.push(id);
            }
        } else {
            users.insert(interaction.user.name.clone(), vec![id]);
        }
        
        let response_string = users.iter().map(|(k, v)| format!("{}: {}",k,v.join(", "))).collect::<Vec<String>>().join("\n");

        interaction.create_response(&ctx, CreateInteractionResponse::UpdateMessage(CreateInteractionResponseMessage::new().content(
            response_string
        ))).await?;
    }

    msg.edit(ctx, EditMessage::new().components(vec![])).await?;
    
    Ok(())
}

#[poise::command(prefix_command)]
pub async fn invite(ctx:Context<'_>) -> Result<(), Error> {
    ctx.say("invite saul NOW https://discord.com/oauth2/authorize?client_id=966122709534797875&scope=bot&permissions=8").await?;
    Ok(())
}

#[poise::command(prefix_command)]
pub async fn cards_named_finger(ctx:Context<'_>) -> Result<(), Error> {
    let m = crate::CARDS.entries().map(|(x,y)|format!("<:{}:{}>",x,y)).collect::<Vec<String>>().join(" ");
    ctx.say(m).await?;
    Ok(())
}

// #[poise::command(prefix_command)]
// pub async fn analyze(ctx:Context<'_>) -> Result<(), Error> {
//     let m = ctx.msg;
//     ctx.say(format!(r#"attachments: {}
// embeds: {}
// nonce: {}
// "#,
//         m.attachments.iter().map(|a| format!("({},{})",a.url,a.content_type.unwrap_or_default())).join(", "),
//         m.embeds.iter().map(|e| e)
//     ))
// }