use anyhow::{Result, anyhow};
use serde_json::Value;

use super::models::*;

fn code_ok(value: &Value) -> bool {
    value.get("code").and_then(as_i64).unwrap_or(200) == 200
}

fn as_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|v| i64::try_from(v).ok()))
        .or_else(|| value.as_str().and_then(|v| v.parse().ok()))
}

fn as_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|v| u64::try_from(v).ok()))
        .or_else(|| value.as_str().and_then(|v| v.parse().ok()))
}

fn as_u32(value: &Value) -> Option<u32> {
    as_u64(value).and_then(|v| u32::try_from(v).ok())
}

fn as_i32(value: &Value) -> Option<i32> {
    as_i64(value).and_then(|v| i32::try_from(v).ok())
}

fn as_bool(value: &Value) -> Option<bool> {
    value.as_bool().or_else(|| match value.as_str()? {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    })
}

fn str_value(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn first_str(value: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .unwrap_or_default()
        .to_string()
}

fn timestamp_to_year(ts_ms: u64) -> Option<u32> {
    if ts_ms == 0 {
        return None;
    }
    let secs = ts_ms / 1000;
    let days = secs / 86400;
    Some(1970 + (days as f64 / 365.25) as u32)
}

fn artists_from_array(items: Option<&Vec<Value>>) -> Vec<ArtistSummary> {
    items
        .into_iter()
        .flatten()
        .filter_map(artist_summary_from_value)
        .collect()
}

pub fn artist_summary_from_value(value: &Value) -> Option<ArtistSummary> {
    let id = value.get("id").and_then(as_u64).unwrap_or_default();
    let name = str_value(value, "name");
    if id == 0 && name.is_empty() {
        return None;
    }

    Some(ArtistSummary {
        id,
        name,
        image_url: first_str(value, &["picUrl", "img1v1Url", "avatar"]),
    })
}

fn user_summary_from_value(value: Option<&Value>) -> UserSummary {
    value
        .map(|user| UserSummary {
            id: user.get("userId").and_then(as_u64).unwrap_or_default(),
            nickname: str_value(user, "nickname"),
            avatar_url: str_value(user, "avatarUrl"),
        })
        .unwrap_or_default()
}

pub fn album_summary_from_value(album: &Value) -> Option<AlbumSummary> {
    let id = album.get("id").and_then(as_u64).unwrap_or_default();
    let name = str_value(album, "name");
    if id == 0 && name.is_empty() {
        return None;
    }

    let artists = album
        .get("artists")
        .and_then(Value::as_array)
        .map(|items| artists_from_array(Some(items)))
        .or_else(|| {
            album
                .get("artist")
                .and_then(artist_summary_from_value)
                .map(|artist| vec![artist])
        })
        .unwrap_or_default();

    Some(AlbumSummary {
        id,
        name,
        image_url: first_str(album, &["picUrl", "pic_url", "coverImgUrl"]),
        artists,
        publish_time: album
            .get("publishTime")
            .and_then(as_u64)
            .unwrap_or_default(),
        tags: str_value(album, "tags"),
    })
}

pub fn playlist_summary_from_value(
    item: &Value,
    default_subscribed: bool,
) -> Option<PlaylistSummary> {
    let id = item.get("id").and_then(as_u64)?;
    Some(PlaylistSummary {
        id,
        name: str_value(item, "name"),
        cover_url: first_str(item, &["coverImgUrl", "picUrl", "coverUrl"]),
        creator: user_summary_from_value(item.get("creator")),
        subscribed: item
            .get("subscribed")
            .and_then(as_bool)
            .unwrap_or(default_subscribed),
    })
}

pub fn track_from_value(track: &Value, album_override: Option<&Value>) -> Result<Track> {
    let artists = {
        let artists = artists_from_array(track.get("ar").and_then(Value::as_array));
        if artists.is_empty() {
            artists_from_array(track.get("artists").and_then(Value::as_array))
        } else {
            artists
        }
    };

    let album_value = album_override
        .or_else(|| track.get("al"))
        .or_else(|| track.get("album"));
    let mut album = album_value
        .and_then(album_summary_from_value)
        .unwrap_or_default();
    if album.artists.is_empty() {
        album.artists = artists.clone();
    }

    let publish_ts = track
        .get("publishTime")
        .and_then(as_u64)
        .or_else(|| {
            album_value
                .and_then(|album| album.get("publishTime"))
                .and_then(as_u64)
        })
        .unwrap_or(album.publish_time);
    let privilege = track.get("privilege");

    Ok(Track {
        id: track
            .get("id")
            .and_then(as_u64)
            .ok_or_else(|| anyhow!("track id missing"))?,
        title: str_value(track, "name"),
        artists,
        album,
        duration_ms: track
            .get("dt")
            .or_else(|| track.get("duration"))
            .and_then(as_u64)
            .unwrap_or_default(),
        availability: privilege
            .map(|value| {
                let st = value.get("st").and_then(as_i32).unwrap_or_default();
                let fee = value.get("fee").and_then(as_i32).unwrap_or_default();
                TrackAvailability::from_privilege(st, fee)
            })
            .unwrap_or(TrackAvailability::Unknown),
        track_number: track.get("no").and_then(as_u32),
        year: timestamp_to_year(publish_ts),
        genre: album_value
            .and_then(|album| album.get("tags"))
            .and_then(Value::as_str)
            .filter(|tags| !tags.is_empty())
            .map(ToString::to_string),
    })
}

pub fn tracks_from_array(items: Option<&Vec<Value>>, album_override: Option<&Value>) -> Vec<Track> {
    items
        .into_iter()
        .flatten()
        .filter_map(|track| track_from_value(track, album_override).ok())
        .collect()
}

pub fn track_urls(value: &Value) -> Result<Vec<TrackUrl>> {
    if !code_ok(value) {
        return Err(anyhow!("track url request failed"));
    }
    Ok(value
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let url = item.get("url").and_then(Value::as_str).unwrap_or_default();
            (!url.is_empty()).then(|| TrackUrl {
                id: item.get("id").and_then(as_u64).unwrap_or_default(),
                url: url.to_string(),
                rate: item
                    .get("br")
                    .or_else(|| item.get("rate"))
                    .and_then(as_u32)
                    .unwrap_or_default(),
            })
        })
        .collect())
}

pub fn track_detail(value: &Value) -> Result<Vec<Track>> {
    if !code_ok(value) {
        return Err(anyhow!("track detail request failed"));
    }
    Ok(tracks_from_array(
        value.get("songs").and_then(Value::as_array),
        None,
    ))
}

pub fn lyrics(value: &Value) -> Result<Lyrics> {
    if !code_ok(value) {
        return Err(anyhow!("lyrics request failed"));
    }

    let split_lyric = |key: &str| {
        value
            .get(key)
            .and_then(|node| node.get("lyric"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .lines()
            .filter(|line| !line.is_empty())
            .map(ToString::to_string)
            .collect()
    };

    Ok(Lyrics {
        lyric: split_lyric("lrc"),
        tlyric: split_lyric("tlyric"),
    })
}

pub fn playlist_detail(value: &Value) -> Result<PlaylistDetail> {
    if !code_ok(value) {
        return Err(anyhow!("playlist detail request failed"));
    }
    let playlist = value
        .get("playlist")
        .ok_or_else(|| anyhow!("playlist missing"))?;
    let tracks = playlist.get("tracks").and_then(Value::as_array);
    let privileges = value.get("privileges").and_then(Value::as_array);

    let mut mapped_tracks = Vec::new();
    for (idx, track) in tracks.into_iter().flatten().enumerate() {
        let mut mapped = track_from_value(track, None)?;
        if let Some(privilege) = privileges.and_then(|items| items.get(idx)) {
            let st = privilege.get("st").and_then(as_i32).unwrap_or_default();
            let fee = privilege.get("fee").and_then(as_i32).unwrap_or_default();
            mapped.availability = TrackAvailability::from_privilege(st, fee);
        }
        mapped_tracks.push(mapped);
    }

    Ok(PlaylistDetail {
        id: playlist.get("id").and_then(as_u64).unwrap_or_default(),
        name: str_value(playlist, "name"),
        cover_url: str_value(playlist, "coverImgUrl"),
        description: str_value(playlist, "description"),
        create_time: playlist
            .get("createTime")
            .and_then(as_u64)
            .unwrap_or_default(),
        track_update_time: playlist
            .get("trackUpdateTime")
            .and_then(as_u64)
            .unwrap_or_default(),
        creator: user_summary_from_value(playlist.get("creator")),
        track_count: playlist
            .get("trackCount")
            .and_then(as_u64)
            .unwrap_or(mapped_tracks.len() as u64),
        subscribed: playlist
            .get("subscribed")
            .and_then(as_bool)
            .unwrap_or_default(),
        tracks: mapped_tracks,
    })
}

pub fn playlist_track_ids(value: &Value) -> Vec<u64> {
    value
        .get("playlist")
        .and_then(|playlist| playlist.get("trackIds"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|track| track.get("id").and_then(as_u64))
        .collect()
}

pub fn artist_detail(value: &Value) -> Result<ArtistDetail> {
    if !code_ok(value) {
        return Err(anyhow!("artist detail request failed"));
    }

    let artist = value
        .get("artist")
        .or_else(|| value.get("data").and_then(|data| data.get("artist")))
        .or_else(|| value.get("data"))
        .ok_or_else(|| anyhow!("artist missing"))?;

    let tracks = tracks_from_array(value.get("hotSongs").and_then(Value::as_array), None);

    Ok(ArtistDetail {
        id: artist.get("id").and_then(as_u64).unwrap_or_default(),
        name: str_value(artist, "name"),
        image_url: first_str(artist, &["picUrl", "avatar", "img1v1Url"]),
        description: artist
            .get("briefDesc")
            .or_else(|| artist.get("desc"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        track_count: artist
            .get("musicSize")
            .and_then(as_u32)
            .unwrap_or(tracks.len() as u32),
        album_count: artist.get("albumSize").and_then(as_u32).unwrap_or_default(),
        mv_count: artist.get("mvSize").and_then(as_u32).unwrap_or_default(),
        followed: artist.get("followed").and_then(as_bool).unwrap_or_default(),
        top_tracks: tracks,
    })
}

pub fn album_detail(value: &Value) -> Result<AlbumDetail> {
    if !code_ok(value) {
        return Err(anyhow!("album detail request failed"));
    }

    let album = value.get("album").ok_or_else(|| anyhow!("album missing"))?;
    let summary = album_summary_from_value(album).unwrap_or_default();
    let tracks = tracks_from_array(value.get("songs").and_then(Value::as_array), Some(album));

    Ok(AlbumDetail {
        id: summary.id,
        name: summary.name,
        image_url: summary.image_url,
        description: album
            .get("description")
            .or_else(|| album.get("briefDesc"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        artists: summary.artists,
        track_count: album
            .get("size")
            .and_then(as_u32)
            .unwrap_or(tracks.len() as u32),
        publish_time: summary.publish_time,
        tracks,
        tags: summary.tags,
    })
}

pub fn user_detail(value: &Value) -> Result<UserDetail> {
    if !code_ok(value) {
        return Err(anyhow!("user detail request failed"));
    }
    let profile = value
        .get("profile")
        .ok_or_else(|| anyhow!("profile missing"))?;

    Ok(UserDetail {
        user_id: profile.get("userId").and_then(as_u64).unwrap_or_default(),
        artist_id: profile.get("artistId").and_then(as_u64).unwrap_or_default(),
        nickname: str_value(profile, "nickname"),
        artist_name: str_value(profile, "artistName"),
        signature: str_value(profile, "signature"),
        follows: profile.get("follows").and_then(as_u64).unwrap_or_default(),
        followeds: profile
            .get("followeds")
            .and_then(as_u64)
            .unwrap_or_default(),
        avatar_url: str_value(profile, "avatarUrl"),
        background_url: str_value(profile, "backgroundUrl"),
    })
}

pub fn playlist_summaries(value: &Value, source: PlaylistSource) -> Result<Vec<PlaylistSummary>> {
    if !code_ok(value) {
        return Err(anyhow!("playlist summary request failed"));
    }

    let items = match source {
        PlaylistSource::User => value.get("playlist").and_then(Value::as_array),
        PlaylistSource::Recommend => value
            .get("recommend")
            .or_else(|| value.get("data"))
            .and_then(Value::as_array),
        PlaylistSource::Top => value.get("playlists").and_then(Value::as_array),
        PlaylistSource::Search => value
            .get("result")
            .and_then(|result| result.get("playlists"))
            .and_then(Value::as_array),
    };

    Ok(items
        .into_iter()
        .flatten()
        .filter_map(|item| {
            playlist_summary_from_value(item, matches!(source, PlaylistSource::User))
        })
        .collect())
}

#[derive(Debug, Clone, Copy)]
pub enum PlaylistSource {
    User,
    Recommend,
    Top,
    Search,
}

pub fn album_summaries(value: &Value, source: AlbumSource) -> Result<Vec<AlbumSummary>> {
    if !code_ok(value) {
        return Err(anyhow!("album summary request failed"));
    }

    let items = match source {
        AlbumSource::Search => value
            .get("result")
            .and_then(|result| result.get("albums"))
            .and_then(Value::as_array),
        AlbumSource::Artist => value.get("hotAlbums").and_then(Value::as_array),
    };

    Ok(items
        .into_iter()
        .flatten()
        .filter_map(album_summary_from_value)
        .collect())
}

#[derive(Debug, Clone, Copy)]
pub enum AlbumSource {
    Search,
    Artist,
}

pub fn artist_summaries(value: &Value) -> Result<Vec<ArtistSummary>> {
    if !code_ok(value) {
        return Err(anyhow!("artist summary request failed"));
    }

    Ok(value
        .get("result")
        .and_then(|result| result.get("artists"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(artist_summary_from_value)
        .collect())
}

pub fn liked_song_ids(value: &Value) -> Result<Vec<u64>> {
    if !code_ok(value) {
        return Err(anyhow!("liked song ids request failed"));
    }
    Ok(value
        .get("ids")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(as_u64)
        .collect())
}

pub fn login_info(value: &Value) -> Result<LoginInfo> {
    let code = value.get("code").and_then(as_i32).unwrap_or_default();
    if code != 200 {
        return Ok(LoginInfo {
            code,
            user_id: 0,
            nickname: String::new(),
            avatar_url: String::new(),
            vip_type: 0,
            msg: value
                .get("msg")
                .or_else(|| value.get("message"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        });
    }

    let profile = value
        .get("profile")
        .ok_or_else(|| anyhow!("login profile missing"))?;

    Ok(LoginInfo {
        code,
        user_id: profile.get("userId").and_then(as_u64).unwrap_or_default(),
        nickname: str_value(profile, "nickname"),
        avatar_url: str_value(profile, "avatarUrl"),
        vip_type: profile.get("vipType").and_then(as_i32).unwrap_or_default(),
        msg: String::new(),
    })
}

pub fn msg(value: &Value) -> Msg {
    Msg {
        code: value.get("code").and_then(as_i32).unwrap_or_default(),
        msg: value
            .get("msg")
            .or_else(|| value.get("message"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    }
}

pub fn banners(value: &Value) -> Result<Vec<Banner>> {
    if !code_ok(value) {
        return Err(anyhow!("banner request failed"));
    }
    Ok(value
        .get("banners")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(Banner {
                image_url: item.get("imageUrl")?.as_str()?.to_string(),
                target_id: item.get("targetId").and_then(as_u64).unwrap_or_default(),
                target: BannerTarget::from(
                    item.get("targetType").and_then(as_i32).unwrap_or_default(),
                ),
                title: str_value(item, "typeTitle"),
            })
        })
        .collect())
}

pub fn search(value: &Value, search_type: SearchType) -> Result<SearchResponse> {
    if !code_ok(value) {
        return Err(anyhow!("search request failed"));
    }
    let result = value.get("result").unwrap_or(&Value::Null);
    let mut response = SearchResponse::default();

    match search_type {
        SearchType::Songs => {
            response.track_count = result.get("songCount").and_then(as_u32).unwrap_or_default();
            response.tracks =
                tracks_from_array(result.get("songs").and_then(Value::as_array), None);
        }
        SearchType::Albums => {
            response.album_count = result
                .get("albumCount")
                .and_then(as_u32)
                .unwrap_or_default();
            response.albums = album_summaries(value, AlbumSource::Search)?;
        }
        SearchType::Artists => {
            response.artist_count = result
                .get("artistCount")
                .and_then(as_u32)
                .unwrap_or_default();
            response.artists = artist_summaries(value)?;
        }
        SearchType::Playlists => {
            response.playlist_count = result
                .get("playlistCount")
                .and_then(as_u32)
                .unwrap_or_default();
            response.playlists = playlist_summaries(value, PlaylistSource::Search)?;
        }
    }

    Ok(response)
}
