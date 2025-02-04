use std::time::Duration;

use anyhow::Error;
use owo_colors::OwoColorize;
use termion::{clear, cursor};
use tunein::TuneInClient;

use crate::{decoder::Mp3Decoder, extract::extract_stream_url};

const METER_CHAR: char = '█';

pub async fn exec(name_or_id: &str) -> Result<(), Error> {
    let client = TuneInClient::new();
    let results = client
        .get_station(name_or_id)
        .await
        .map_err(|e| Error::msg(e.to_string()))?;
    let (url, playlist_type, _) = match results.is_empty() {
        true => {
            let results = client
                .search(name_or_id)
                .await
                .map_err(|e| Error::msg(e.to_string()))?;
            match results.first() {
                Some(result) => {
                    if result.r#type != Some("audio".to_string()) {
                        return Err(Error::msg("No station found"));
                    }
                    let id = result.guide_id.as_ref().unwrap();
                    let station = client
                        .get_station(id)
                        .await
                        .map_err(|e| Error::msg(e.to_string()))?;
                    let station = station.first().unwrap();
                    (
                        station.url.clone(),
                        station.playlist_type.clone(),
                        station.media_type.clone(),
                    )
                }
                None => ("".to_string(), None, "".to_string()),
            }
        }
        false => {
            let result = results.first().unwrap();
            (
                result.url.clone(),
                result.playlist_type.clone(),
                result.media_type.clone(),
            )
        }
    };
    let stream_url = extract_stream_url(&url, playlist_type).await?;
    println!("{}", stream_url);

    tokio::task::spawn_blocking(move || {
        let client = reqwest::blocking::Client::new();

        let response = client.get(stream_url).send().unwrap();

        println!("headers: {:#?}", response.headers());
        let location = response.headers().get("location");

        let response = match location {
            Some(location) => {
                let response = client.get(location.to_str().unwrap()).send().unwrap();
                let location = response.headers().get("location");
                match location {
                    Some(location) => client.get(location.to_str().unwrap()).send().unwrap(),
                    None => response,
                }
            }
            None => response,
        };

        let (_stream, handle) = rodio::OutputStream::try_default().unwrap();
        let sink = rodio::Sink::try_new(&handle).unwrap();
        let decoder = Mp3Decoder::new(response).unwrap();
        sink.append(decoder);

        loop {
            let level = sink.volume();
            display_vu_meter(level);
            std::thread::sleep(Duration::from_millis(10));
        }
    })
    .await?;

    Ok(())
}

fn display_vu_meter(level: f32) {
    print!("{}{}", clear::All, cursor::Goto(1, 1));
    for i in 0..20 {
        if (i as f32) / 20.0 <= level {
            print!("{}", METER_CHAR.bright_yellow());
        } else {
            print!("{}", METER_CHAR.bright_yellow());
        }
    }
}
