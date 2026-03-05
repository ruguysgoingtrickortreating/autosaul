use itertools::Itertools;
use poise::CreateReply;
use poise::serenity_prelude::CreateAttachment;
use saulimages::Operation;
use crate::{Context, Error, REQ_CLIENT, image_edit, serenity};
use serenity::{GetMessages, Message};
use tokio::io::AsyncBufReadExt;

async fn message_attachment_fetch(ctx: &Context<'_>) -> Result<Option<Vec<u8>>, Error> {
    let m_finder = |m: &Message| -> Option<String> {
        let f = m.attachments.iter().find_map(|a| {
            let Some(c_t) = &a.content_type else {return None};
            let mut info = c_t.split("/");
            if let Some(im) = info.next() && im == "image" {
                if let Some(kind) = info.next() {
                    match kind {
                        "jpeg" | "png" | "webp" | "gif" | "avif" =>
                            return Some(a.url.clone()),
                        _ => ()
                    }
                }
            }
            None
        });
        if f.is_some() {return f}
        let i = if let Some(u) = m.content.find("https://") {u}
            else if let Some(u) = m.content.find("http://") {u}
            else {return None};
        Some(
            if let Some(i2) = m.content[i..].find(' ') {
                m.content[i..i2].to_string()
            } else {
                m.content[i..].to_string()
            }
        )
    };

    let mut url = 'image: {
        if let Some(message) = &ctx.msg.referenced_message {
            let Some(img) = m_finder(message) else {
                ctx.say("message had no valid images 😿").await?;
                return Ok(None);
            };
            break 'image img;
        }
        let messages = ctx.channel_id().messages(&ctx, GetMessages::new().limit(10)).await?;
        let Some(img) = messages.into_iter().find_map(|m| m_finder(&m)) else {
            ctx.say("last 10 messages had no valid images. try replying to the image you want").await?;
            return Ok(None);
        };
        img
    };
    if let Some(i) = url.find("://tenor.com/view") {
        let req = REQ_CLIENT.get(&url).send().await?;
        if !req.status().is_success() {
            return Err(req.error_for_status().unwrap_err().into());
        }
        let text = req.text().await?;
        let Some(i1) = text.find("https://media1.tenor.com/m/") else {return Ok(None)};
        let i2 = text[i1..].find(".gif").unwrap();
        url = text[i1..i1+i2+4].to_string();
    }

    let req = REQ_CLIENT.get(url).send().await?;
    if !req.status().is_success() {
        Err(req.error_for_status().unwrap_err().into())
    } else {
        Ok(Some(req.bytes().await?.to_vec()))
    }
}

#[poise::command(prefix_command, category = "images")]
pub async fn chain(ctx: Context<'_>, #[rest] operations:String)-> Result<(), Error> {
    let Some(img) = message_attachment_fetch(&ctx).await? else {return Ok(())};

    if operations.is_empty() {
        ctx.say("provide image commands to chain").await?;
        return Ok(());
    }

    let mut operations_list: Vec<(String, Option<String>)> = vec![];
    let mut curlybrace_layers = 0;
    let mut string_buffer = String::new();

    let mut slice = operations.split(" ");
    while let Some(s) = slice.next() {

        if s.starts_with('{') {
            let op;
            let mut args = vec![];
            curlybrace_layers += 1;
            if s == "{" {
                if curlybrace_layers == 1 && let Some(n) = slice.next() {
                    op = n;
                } else {
                    ctx.say("unclosed brace 😿").await?;
                    return Ok(());
                }
            } else {
                op = &s[1..];
            }
            let mut ended = false;
            while let Some(n) = slice.next() {
                if n.ends_with('}') {
                    curlybrace_layers -= 1;
                    if curlybrace_layers == 0 {
                        if !(n == "}") {
                            args.push(&n[0..n.len()-1]);
                        }
                        ended = true;
                        break
                    }
                }
                args.push(n);
            }
            if !ended {
                ctx.say("unclosed brace 😿").await?;
                return Ok(());
            }
            operations_list.push((op.to_string(), Some(args.join(" "))));
        } else {
            operations_list.push((s.to_string(), None));
        }
    }

    println!("{:?}", operations_list);

    // validate
    for (op, args) in &operations_list {
        match op.as_str() {
            "caption" => {
                if args.is_none() || args.as_ref().unwrap().is_empty() {
                    ctx.say("no caption provided").await?;
                    return Ok(());
                }
            }
            "pugsley" | "rio_de_janeiro" | "riodejaneiro" | "papyrus" => (),
            _ => {
                ctx.say(format!("unknown image command: {op}")).await?;
                return Ok(());
            }
        }
    }

    let reply = ctx.say("<a:callsaul:970877544473710602>:chains:<a:callsaul:970877544473710602> chaining...").await?.into_message().await?;

    // execute
    let ops_to_execute = operations_list.into_iter().map( |(op, args)|
        match op.to_lowercase().as_str() {
            "caption" => Operation::Caption(args.unwrap()),
            "pugsley" => Operation::Pugsley,
            "rio_de_janeiro" | "riodejaneiro" => Operation::RioDeJaneiro,
            "papyrus" => Operation::Papyrus(args.unwrap()),
            _ => unreachable!(),
        }
    ).collect_vec();
    let chain =
        match image_edit::chain(img, ops_to_execute).await {
            Ok(ok) => {ok}
            Err(err) => {
                reply.delete(&ctx).await?;
                ctx.say(format!("chaining failed. this is because you are doing Bullshit.\n{err}")).await?;
                return Ok(());
            }
        };
    let attachment = CreateAttachment::bytes(chain, format!("{}.webp", ctx.msg.id));
    ctx.send(CreateReply::default().attachment(attachment)).await?;
    reply.delete(&ctx).await?;

    // let mut index = 0;
    // let mut next_index = 0;
    // loop {
    //
    //     let slice = if let Some(f) = operations.find(' ') {
    //         next_index = f;
    //         &operations[index..next_index]
    //     } else {
    //         next_index = index;
    //         &operations[index..]
    //     };
    //     if slice.starts_with('{') {
    //         curlybrace_layers += 1;
    //         let op;
    //         let args;
    //         if slice == "{" {
    //             let ind = index + 1;
    //         } else {
    //             brace_start_index = index + 1;
    //         }
    //     } else {
    //         operations_list.push((slice, None))
    //     }
    // }

    Ok(())
}

#[poise::command(prefix_command, category = "images")]
pub async fn caption(ctx: Context<'_>, #[rest] caption: String) -> Result<(), Error> {
    let Some(img) = message_attachment_fetch(&ctx).await? else {return Ok(())};
    let reply = ctx.say("<a:callsaul:970877544473710602> captioning...").await?.into_message().await?;
    let output = image_edit::caption(img, caption).await?;
    let attachment = CreateAttachment::bytes(output, format!("{}.webp", ctx.msg.id));
    ctx.send(CreateReply::default().attachment(attachment)).await?;
    reply.delete(&ctx).await?;

    Ok(())
}

#[poise::command(prefix_command, category = "images")]
pub async fn pugsley(ctx:Context<'_>) -> Result<(), Error> {
    let Some(img) = message_attachment_fetch(&ctx).await? else {return Ok(())};
    let reply = ctx.say("<a:callsaul:970877544473710602> pugsleying...").await?.into_message().await?;
    let output = image_edit::pugsley(img).await?;
    let attachment = CreateAttachment::bytes(output, format!("{}.webp", ctx.msg.id));
    ctx.send(CreateReply::default().attachment(attachment)).await?;
    reply.delete(&ctx).await?;

    Ok(())
}

#[poise::command(prefix_command, category = "images", aliases("riodejaneiro"))]
pub async fn rio_de_janeiro(ctx:Context<'_>) -> Result<(), Error> {
    let Some(img) = message_attachment_fetch(&ctx).await? else {return Ok(())};
    let reply = ctx.say("<a:callsaul:970877544473710602> rio de janeiro...").await?.into_message().await?;
    let output = image_edit::rio_de_janeiro(img).await?;
    let attachment = CreateAttachment::bytes(output, format!("{}.webp", ctx.msg.id));
    ctx.send(CreateReply::default().attachment(attachment)).await?;
    reply.delete(&ctx).await?;

    Ok(())
}

#[poise::command(prefix_command, category = "images")]
pub async fn papyrus(ctx: Context<'_>, #[rest] caption: String) -> Result<(), Error> {
    let Some(img) = message_attachment_fetch(&ctx).await? else {return Ok(())};
    let reply = ctx.say("<a:callsaul:970877544473710602> captioning...").await?.into_message().await?;
    let output = image_edit::papyrus(img, caption).await?;
    let attachment = CreateAttachment::bytes(output, format!("{}.webp", ctx.msg.id));
    ctx.send(CreateReply::default().attachment(attachment)).await?;
    reply.delete(&ctx).await?;

    Ok(())
}

#[poise::command(prefix_command, category = "images")]
pub async fn burn(ctx:Context<'_>) -> Result<(), Error> {
    let Some(img) = message_attachment_fetch(&ctx).await? else {return Ok(())};
    let reply = ctx.say("<a:callsaul:970877544473710602> burning...").await?.into_message().await?;
    let output = image_edit::burn(img).await?;
    let attachment = CreateAttachment::bytes(output, format!("{}.webp", ctx.msg.id));
    ctx.send(CreateReply::default().attachment(attachment)).await?;
    reply.delete(&ctx).await?;

    Ok(())
}