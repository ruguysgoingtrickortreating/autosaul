use poise::serenity_prelude as serenity;
use std::fs::File;
use std::io::{BufWriter, Write};
use project_root;

#[tokio::main]
async fn main() {
    let token = include_str!("../../token.txt");

    let path = project_root::get_project_root().expect("couldn't get project root").join("cards_map.rs");
    let mut file = BufWriter::new(File::create(&path).expect("couldnt create file"));
    let http = serenity::Http::new(token);
    http.set_application_id(966122709534797875.into());

    let mut map = phf_codegen::Map::new();

    let emojis = http.get_application_emojis().await.unwrap();
    for e in emojis {
        map.entry(e.name, format!("serenity::EmojiId::new({})",e.id));
    }
    

    write!(&mut file, "\n
//////////////////////////////////////////////////////////////\n
// generated using emoji_scrape_tool.rs in /bin             //\n
// you will need 52 card emojis in your application emojis, //\n
// named like clubs_2 or spades_A (capitalize the letters)  //\n
//////////////////////////////////////////////////////////////\n
\n
static CARDS: phf::Map<&'static str, serenity::EmojiId> = {}",
        map.build())
    .unwrap();
    write!(&mut file, ";\n").unwrap();
}