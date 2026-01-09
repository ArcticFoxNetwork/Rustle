//! NCM API Model types
//!
//! Data structures for NCM API responses.

use anyhow::{Context, Ok, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::Value;

trait DeVal<'a>: Sized {
    fn dval(v: &'a Value) -> Result<Self>;
}

impl<'a> DeVal<'a> for bool {
    fn dval(v: &Value) -> Result<Self> {
        Ok(Self::deserialize(v)?)
    }
}

impl<'a> DeVal<'a> for i64 {
    fn dval(v: &Value) -> Result<Self> {
        Ok(Self::deserialize(v)?)
    }
}

impl<'a> DeVal<'a> for u64 {
    fn dval(v: &Value) -> Result<Self> {
        Ok(Self::deserialize(v)?)
    }
}

impl<'a> DeVal<'a> for i32 {
    fn dval(v: &Value) -> Result<Self> {
        Ok(Self::deserialize(v)?)
    }
}

impl<'a> DeVal<'a> for u32 {
    fn dval(v: &Value) -> Result<Self> {
        Ok(Self::deserialize(v)?)
    }
}

impl<'a> DeVal<'a> for String {
    fn dval(v: &Value) -> Result<Self> {
        Ok(Self::deserialize(v)?)
    }
}

impl<'a> DeVal<'a> for &'a Vec<Value> {
    fn dval(v: &'a Value) -> Result<Self> {
        match v {
            Value::Array(v) => Ok(v),
            _ => Err(anyhow!("json not a array")),
        }
    }
}

impl<'a> DeVal<'a> for &'a Value {
    fn dval(v: &'a Value) -> Result<Self> {
        Ok(v)
    }
}

fn get_val_chain<'a, T>(v: &'a Value, names: &[&str]) -> Result<T>
where
    T: DeVal<'a>,
{
    let v = names.iter().fold(std::result::Result::Ok(v), |v, n| {
        v?.get(n)
            .ok_or_else(|| anyhow!("key '{}' not found, in chain {:?}", n, names))
    })?;
    Ok(T::dval(v)?)
}

macro_rules! get_val {
    (@as $t:ty, $v:expr, $($n:expr),+) => {
        get_val_chain::<$t>($v, &[$($n),+]).context(format!("at {}:{}", file!(), line!()))
    };
    ($v:expr, $($n:expr),+) => {
        get_val_chain($v, &[$($n),+]).context(format!("at {}:{}", file!(), line!()))
    };
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Lyrics {
    pub lyric: Vec<String>,
    pub tlyric: Vec<String>,
}

pub fn to_lyric(json: String) -> Result<Lyrics> {
    let value = &serde_json::from_str::<Value>(&json)?;
    let code: i64 = get_val!(value, "code")?;
    if code == 200 {
        let lrc: String = get_val!(value, "lrc", "lyric")?;
        let lyric = lrc
            .split('\n')
            .collect::<Vec<&str>>()
            .iter()
            .map(|s| (*s).to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<String>>();
        let lrc: String = get_val!(value, "tlyric", "lyric")?;
        let tlyric = lrc
            .split('\n')
            .collect::<Vec<&str>>()
            .iter()
            .map(|s| (*s).to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<String>>();
        return Ok(Lyrics { lyric, tlyric });
    }
    Err(anyhow!("none"))
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SingerInfo {
    pub id: u64,
    pub name: String,
    pub pic_url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SongUrl {
    pub id: u64,
    pub url: String,
    pub rate: u32,
}

pub fn to_song_url(json: String) -> Result<Vec<SongUrl>> {
    let value = &serde_json::from_str::<Value>(&json)?;
    let code: i64 = get_val!(value, "code")?;
    if code == 200 {
        let mut vec: Vec<SongUrl> = Vec::new();
        let array: &Vec<Value> = get_val!(value, "data")?;
        for v in array.iter() {
            let url: String = get_val!(v, "url").unwrap_or_default();
            if !url.is_empty() {
                vec.push(SongUrl {
                    id: get_val!(v, "id")?,
                    url,
                    rate: get_val!(v, "br")?,
                });
            }
        }
        return Ok(vec);
    }
    Err(anyhow!("none"))
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub enum SongCopyright {
    Free,
    VipOnly,
    Payment,
    VipOnlyHighRate,
    Unavailable,
    Unknown,
}

impl SongCopyright {
    fn from_fee(fee: i32) -> Self {
        match fee {
            0 => Self::Free,
            1 => Self::VipOnly,
            4 => Self::Payment,
            8 => Self::VipOnlyHighRate,
            _ => Self::Unknown,
        }
    }

    pub fn from_privilege(v: &Value) -> Result<Self> {
        let st: i32 = get_val!(v, "st")?;
        let fee: i32 = get_val!(v, "fee")?;

        let res = if st < 0 {
            Self::Unavailable
        } else {
            Self::from_fee(fee)
        };
        Ok(res)
    }

    pub fn playable(&self) -> bool {
        self != &Self::Unavailable
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SongInfo {
    pub id: u64,
    pub name: String,
    pub singer: String,
    pub album: String,
    pub album_id: u64,
    pub pic_url: String,
    pub duration: u64,
    pub song_url: String,
    pub copyright: SongCopyright,
}

impl PartialEq for SongInfo {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Default for SongInfo {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            singer: String::new(),
            album: String::new(),
            album_id: 0,
            pic_url: String::new(),
            duration: 0,
            song_url: String::new(),
            copyright: SongCopyright::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Parse {
    Usl,
    Ucd,
    Rmd,
    Rmds,
    Search,
    SearchAlbum,
    LikeAlbum,
    Sd,
    Album,
    Top,
    Singer,
    SingerSongs,
    Radio,
    Intelligence,
    PersonalFm,
}

pub fn to_song_info(json: String, parse: Parse) -> Result<Vec<SongInfo>> {
    let value = &serde_json::from_str::<Value>(&json)?;
    let code: i64 = get_val!(value, "code")?;

    let unk = "unknown".to_string();
    if code == 200 {
        let mut vec: Vec<SongInfo> = Vec::new();
        let list = vec![];
        match parse {
            Parse::Usl => {
                let mut array: &Vec<Value> = get_val!(value, "songs").unwrap_or(&list);
                if array.is_empty() {
                    array = get_val!(value, "playlist", "tracks")?;
                }
                for v in array.iter() {
                    vec.push(SongInfo {
                        id: get_val!(v, "id")?,
                        name: get_val!(v, "name")?,
                        singer: get_val!(@as &Vec<Value>, v, "ar")?
                            .first()
                            .map(|v: &Value| get_val!(v, "name").unwrap_or_else(|_| unk.clone()))
                            .unwrap_or_else(|| unk.clone()),
                        album: get_val!(v, "al", "name").unwrap_or_else(|_| unk.clone()),
                        album_id: get_val!(v, "al", "id")?,
                        pic_url: get_val!(v, "al", "picUrl").unwrap_or_default(),
                        duration: get_val!(v, "dt")?,
                        song_url: String::new(),
                        copyright: SongCopyright::Unknown,
                    });
                }
            }
            Parse::Ucd => {
                let array: &Vec<Value> = get_val!(value, "data")?;
                for v in array.iter() {
                    vec.push(SongInfo {
                        id: get_val!(v, "songId")?,
                        name: get_val!(v, "songName")?,
                        singer: get_val!(v, "artist").unwrap_or_else(|_| unk.clone()),
                        album: get_val!(v, "album").unwrap_or_else(|_| unk.clone()),
                        album_id: 0,
                        pic_url: get_val!(v, "simpleSong", "al", "picUrl").unwrap_or_default(),
                        duration: get_val!(v, "simpleSong", "dt")?,
                        song_url: String::new(),
                        copyright: SongCopyright::Unknown,
                    });
                }
            }
            Parse::Rmd => {
                let array: &Vec<Value> = get_val!(value, "data")?;
                for v in array.iter() {
                    vec.push(SongInfo {
                        id: get_val!(v, "id")?,
                        name: get_val!(v, "name")?,
                        singer: get_val!(@as &Vec<Value>, v, "artists")?
                            .first()
                            .map(|v: &Value| get_val!(v, "name").unwrap_or_else(|_| unk.clone()))
                            .unwrap_or_else(|| unk.clone()),
                        album: get_val!(v, "album", "name").unwrap_or_else(|_| unk.clone()),
                        album_id: get_val!(v, "album", "id")?,
                        pic_url: get_val!(v, "album", "picUrl").unwrap_or_default(),
                        duration: get_val!(v, "duration")?,
                        song_url: String::new(),
                        copyright: SongCopyright::Unknown,
                    });
                }
            }
            Parse::Rmds => {
                let array: &Vec<Value> = get_val!(value, "data", "dailySongs")?;
                for v in array.iter() {
                    vec.push(SongInfo {
                        id: get_val!(v, "id")?,
                        name: get_val!(v, "name")?,
                        singer: get_val!(@as &Vec<Value>, v, "artists")?
                            .first()
                            .map(|v: &Value| get_val!(v, "name").unwrap_or_else(|_| unk.clone()))
                            .unwrap_or_else(|| unk.clone()),
                        album: get_val!(v, "album", "name").unwrap_or_else(|_| unk.clone()),
                        album_id: get_val!(v, "album", "id")?,
                        pic_url: get_val!(v, "album", "picUrl").unwrap_or_default(),
                        duration: get_val!(v, "duration")?,
                        song_url: String::new(),
                        copyright: SongCopyright::Unknown,
                    });
                }
            }
            Parse::Search => {
                let array: &Vec<Value> = get_val!(value, "result", "songs")?;
                for v in array.iter() {
                    vec.push(SongInfo {
                        id: get_val!(v, "id")?,
                        name: get_val!(v, "name")?,
                        singer: get_val!(@as &Vec<Value>, v, "artists")?
                            .first()
                            .map(|v: &Value| get_val!(v, "name").unwrap_or_else(|_| unk.clone()))
                            .unwrap_or_else(|| unk.clone()),
                        album: get_val!(v, "album", "name").unwrap_or_else(|_| unk.clone()),
                        album_id: get_val!(v, "album", "id")?,
                        pic_url: get_val!(v, "album", "picUrl").unwrap_or_default(),
                        duration: get_val!(v, "duration")?,
                        song_url: String::new(),
                        copyright: SongCopyright::Unknown,
                    });
                }
            }
            Parse::Album => {
                let array: &Vec<Value> = get_val!(value, "songs")?;
                for v in array.iter() {
                    vec.push(SongInfo {
                        id: get_val!(v, "id")?,
                        name: get_val!(v, "name")?,
                        singer: get_val!(@as &Vec<Value>, v, "ar")?
                            .first()
                            .map(|v: &Value| get_val!(v, "name").unwrap_or_else(|_| unk.clone()))
                            .unwrap_or_else(|| unk.clone()),
                        album: get_val!(value, "album", "name").unwrap_or_else(|_| unk.clone()),
                        album_id: get_val!(value, "album", "id")?,
                        pic_url: get_val!(value, "album", "picUrl").unwrap_or_default(),
                        duration: get_val!(v, "dt")?,
                        song_url: String::new(),
                        copyright: SongCopyright::Unknown,
                    });
                }
            }
            Parse::Singer => {
                let array: &Vec<Value> = get_val!(value, "hotSongs")?;
                for v in array.iter() {
                    vec.push(SongInfo {
                        id: get_val!(v, "id")?,
                        name: get_val!(v, "name")?,
                        singer: get_val!(value, "artist", "name")?,
                        album: get_val!(v, "al", "name").unwrap_or_else(|_| unk.clone()),
                        album_id: get_val!(v, "al", "id")?,
                        pic_url: get_val!(v, "al", "picUrl").unwrap_or_default(),
                        duration: get_val!(v, "dt")?,
                        song_url: String::new(),
                        copyright: SongCopyright::Unknown,
                    });
                }
            }
            Parse::SingerSongs => {
                let array: &Vec<Value> = get_val!(value, "songs")?;
                for v in array.iter() {
                    vec.push(SongInfo {
                        id: get_val!(v, "id")?,
                        name: get_val!(v, "name")?,
                        singer: get_val!(@as &Vec<Value>, v, "ar")?
                            .first()
                            .map(|v: &Value| get_val!(v, "name").unwrap_or_else(|_| unk.clone()))
                            .unwrap_or_else(|| unk.clone()),
                        album: get_val!(v, "al", "name").unwrap_or_else(|_| unk.clone()),
                        album_id: get_val!(v, "al", "id")?,
                        pic_url: get_val!(v, "al", "picUrl").unwrap_or_default(),
                        duration: get_val!(v, "dt")?,
                        song_url: String::new(),
                        copyright: SongCopyright::Unknown,
                    });
                }
            }
            Parse::Radio => {
                let array: &Vec<Value> = get_val!(value, "programs")?;
                let mut num = array.len();
                for v in array.iter() {
                    vec.push(SongInfo {
                        id: get_val!(v, "mainTrackId")?,
                        name: get_val!(v, "name")?,
                        singer: format!("第 {} 期", num),
                        album: get_val!(@as u64, v, "createTime")?.to_string(),
                        album_id: 0,
                        pic_url: get_val!(v, "coverUrl")?,
                        duration: get_val!(v, "duration")?,
                        song_url: String::new(),
                        copyright: SongCopyright::Unknown,
                    });
                    num -= 1;
                }
            }
            Parse::Intelligence => {
                let array: &Vec<Value> = get_val!(value, "data")?;
                for v in array.iter() {
                    vec.push(SongInfo {
                        id: get_val!(v, "id")?,
                        name: get_val!(v, "songInfo", "name")?,
                        singer: get_val!(@as &Vec<Value>, v, "songInfo", "ar")?
                            .first()
                            .map(|v: &Value| get_val!(v, "name").unwrap_or_else(|_| unk.clone()))
                            .unwrap_or_else(|| unk.clone()),
                        album: get_val!(v, "songInfo", "al", "name")
                            .unwrap_or_else(|_| unk.clone()),
                        album_id: get_val!(v, "songInfo", "al", "id")?,
                        pic_url: get_val!(v, "songInfo", "al", "picUrl").unwrap_or_default(),
                        duration: get_val!(v, "songInfo", "dt")?,
                        song_url: String::new(),
                        copyright: SongCopyright::Unknown,
                    });
                }
            }
            Parse::PersonalFm => {
                // 私人FM返回格式: { data: [{ id, name, artists: [{name}], album: {name, id, picUrl}, duration }] }
                let array: &Vec<Value> = get_val!(value, "data")?;
                for v in array.iter() {
                    vec.push(SongInfo {
                        id: get_val!(v, "id")?,
                        name: get_val!(v, "name")?,
                        singer: get_val!(@as &Vec<Value>, v, "artists")?
                            .first()
                            .map(|v: &Value| get_val!(v, "name").unwrap_or_else(|_| unk.clone()))
                            .unwrap_or_else(|| unk.clone()),
                        album: get_val!(v, "album", "name").unwrap_or_else(|_| unk.clone()),
                        album_id: get_val!(v, "album", "id")?,
                        pic_url: get_val!(v, "album", "picUrl").unwrap_or_default(),
                        duration: get_val!(v, "duration")?,
                        song_url: String::new(),
                        copyright: SongCopyright::Unknown,
                    });
                }
            }
            _ => {}
        }
        return Ok(vec);
    }
    Err(anyhow!("none"))
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlayListDetail {
    pub id: u64,
    pub name: String,
    pub cover_img_url: String,
    pub description: String,
    pub create_time: u64,
    pub track_update_time: u64,
    pub creator_id: u64,
    pub creator_nickname: String,
    pub creator_avatar_url: String,
    pub track_count: u64,
    pub subscribed: bool,
    pub songs: Vec<SongInfo>,
}

pub fn to_mix_detail(json: &Value) -> Result<PlayListDetail> {
    let value = json;
    let code: i64 = get_val!(value, "code")?;
    if code == 200 {
        let unk = "unknown".to_string();

        let mut songs: Vec<SongInfo> = Vec::new();
        let list = vec![];
        let mut array: &Vec<Value> = get_val!(value, "songs").unwrap_or(&list);
        if array.is_empty() {
            array = get_val!(value, "playlist", "tracks")?;
        }
        let array_privilege: &Vec<Value> = get_val!(value, "privileges")?;
        for (v, p) in array.iter().zip(array_privilege.iter()) {
            songs.push(SongInfo {
                id: get_val!(v, "id")?,
                name: get_val!(v, "name")?,
                singer: get_val!(@as &Vec<Value>, v, "ar")?
                    .first()
                    .map(|v: &Value| get_val!(v, "name").unwrap_or_else(|_| unk.clone()))
                    .unwrap_or_else(|| unk.clone()),
                album: get_val!(v, "al", "name").unwrap_or_else(|_| unk.clone()),
                album_id: get_val!(v, "al", "id")?,
                pic_url: get_val!(v, "al", "picUrl").unwrap_or_default(),
                duration: get_val!(v, "dt")?,
                song_url: String::new(),
                copyright: SongCopyright::from_privilege(p)?,
            });
        }

        return Ok(PlayListDetail {
            id: get_val!(value, "playlist", "id")?,
            name: get_val!(value, "playlist", "name")?,
            cover_img_url: get_val!(value, "playlist", "coverImgUrl")?,
            description: get_val!(value, "playlist", "description").unwrap_or_default(),
            create_time: get_val!(value, "playlist", "createTime")?,
            track_update_time: get_val!(value, "playlist", "trackUpdateTime")?,
            creator_id: get_val!(value, "playlist", "creator", "userId").unwrap_or(0),
            creator_nickname: get_val!(value, "playlist", "creator", "nickname")
                .unwrap_or_default(),
            creator_avatar_url: get_val!(value, "playlist", "creator", "avatarUrl")
                .unwrap_or_default(),
            track_count: get_val!(value, "playlist", "trackCount").unwrap_or(songs.len() as u64),
            subscribed: get_val!(value, "playlist", "subscribed").unwrap_or(false),
            songs,
        });
    }
    Err(anyhow!("none"))
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SongList {
    pub id: u64,
    pub name: String,
    pub cover_img_url: String,
    pub author: String,
}

pub fn to_song_list(json: String, parse: Parse) -> Result<Vec<SongList>> {
    let value = serde_json::from_str::<Value>(&json)?;
    if value.get("code").ok_or_else(|| anyhow!("none"))?.eq(&200) {
        let mut vec: Vec<SongList> = Vec::new();
        match parse {
            Parse::Usl => {
                let array: &Vec<Value> = get_val!(&value, "playlist")?;
                for v in array.iter() {
                    vec.push(SongList {
                        id: get_val!(v, "id")?,
                        name: get_val!(v, "name")?,
                        cover_img_url: get_val!(v, "coverImgUrl")?,
                        author: get_val!(v, "creator", "nickname")?,
                    });
                }
            }
            Parse::Rmd => {
                let array: &Vec<Value> = get_val!(&value, "recommend")?;
                for v in array.iter() {
                    vec.push(SongList {
                        id: get_val!(v, "id")?,
                        name: get_val!(v, "name")?,
                        cover_img_url: get_val!(v, "picUrl").unwrap_or_default(),
                        author: get_val!(v, "creator", "nickname")?,
                    });
                }
            }
            Parse::Album => {
                let array: &Vec<Value> = get_val!(&value, "albums")?;
                for v in array.iter() {
                    vec.push(SongList {
                        id: get_val!(v, "id")?,
                        name: get_val!(v, "name")?,
                        cover_img_url: get_val!(v, "picUrl")?,
                        author: get_val!(v, "artist", "name")?,
                    });
                }
            }
            Parse::Top => {
                let array: &Vec<Value> = get_val!(&value, "playlists")?;
                for v in array.iter() {
                    vec.push(SongList {
                        id: get_val!(v, "id")?,
                        name: get_val!(v, "name")?,
                        cover_img_url: get_val!(v, "coverImgUrl")?,
                        author: get_val!(v, "creator", "nickname")?,
                    });
                }
            }
            Parse::Search => {
                let array: &Vec<Value> = get_val!(&value, "result", "playlists")?;
                for v in array.iter() {
                    vec.push(SongList {
                        id: get_val!(v, "id")?,
                        name: get_val!(v, "name")?,
                        cover_img_url: get_val!(v, "coverImgUrl")?,
                        author: get_val!(v, "creator", "nickname")?,
                    });
                }
            }
            Parse::SearchAlbum => {
                let array: &Vec<Value> = get_val!(&value, "result", "albums")?;
                for v in array.iter() {
                    vec.push(SongList {
                        id: get_val!(v, "id")?,
                        name: get_val!(v, "name")?,
                        cover_img_url: get_val!(v, "picUrl")?,
                        author: get_val!(v, "artist", "name")?,
                    });
                }
            }
            Parse::LikeAlbum => {
                let array: &Vec<Value> = get_val!(&value, "data")?;
                for v in array.iter() {
                    vec.push(SongList {
                        id: get_val!(v, "id")?,
                        name: get_val!(v, "name")?,
                        cover_img_url: get_val!(v, "picUrl")?,
                        author: get_val!(@as &Vec<Value>, v, "artists")?
                            .first()
                            .map_or(std::result::Result::Ok(String::new()), |v: &Value| {
                                get_val!(v, "name")
                            })?,
                    });
                }
            }
            Parse::Radio => {
                let array: &Vec<Value> = get_val!(&value, "djRadios")?;
                for v in array.iter() {
                    vec.push(SongList {
                        id: get_val!(v, "id")?,
                        name: get_val!(v, "name")?,
                        cover_img_url: get_val!(v, "picUrl")?,
                        author: get_val!(v, "dj", "nickname")?,
                    });
                }
            }
            _ => {}
        }
        return Ok(vec);
    }
    Err(anyhow!("none"))
}

pub fn to_song_id_list(json: String) -> Result<Vec<u64>> {
    let value = &serde_json::from_str::<Value>(&json)?;
    let code: i64 = get_val!(value, "code")?;
    if code == 200 {
        let id_array: &Vec<Value> = get_val!(value, "ids")?;
        return id_array.iter().map(u64::dval).collect();
    }
    Err(anyhow!("none"))
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Msg {
    pub code: i32,
    pub msg: String,
}

pub fn to_msg(json: String) -> Result<Msg> {
    let value = &serde_json::from_str::<Value>(&json)?;
    let code: i32 = get_val!(value, "code")?;
    if code.eq(&200) {
        return Ok(Msg {
            code: 200,
            msg: "".to_owned(),
        });
    }
    let msg = get_val!(value, "msg")?;
    Ok(Msg { code, msg })
}

pub fn to_message(json: String) -> Result<Msg> {
    let value = &serde_json::from_str::<Value>(&json)?;
    let code: i32 = get_val!(value, "code")?;
    if code.eq(&200) {
        return Ok(Msg {
            code: 200,
            msg: "".to_owned(),
        });
    }

    let msg = get_val!(value, "message")?;
    Ok(Msg { code, msg })
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct LoginInfo {
    pub code: i32,
    pub uid: u64,
    pub nickname: String,
    pub avatar_url: String,
    pub vip_type: i32,
    pub msg: String,
}

pub fn to_login_info(json: String) -> Result<LoginInfo> {
    let value = &serde_json::from_str::<Value>(&json)?;
    let code: i32 = get_val!(value, "code")?;
    if code.eq(&200) {
        return Ok(LoginInfo {
            code,
            uid: get_val!(value, "profile", "userId")?,
            nickname: get_val!(value, "profile", "nickname")?,
            avatar_url: get_val!(value, "profile", "avatarUrl")?,
            vip_type: get_val!(value, "profile", "vipType")?,
            msg: "".to_owned(),
        });
    }

    let msg = get_val!(value, "msg")?;
    Ok(LoginInfo {
        code,
        uid: 0,
        nickname: "".to_owned(),
        avatar_url: "".to_owned(),
        vip_type: 0,
        msg,
    })
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BannersInfo {
    pub pic: String,
    pub target_id: u64,
    pub target_type: TargetType,
    pub type_title: String,
}

pub fn to_banners_info(json: String) -> Result<Vec<BannersInfo>> {
    let value = &serde_json::from_str::<Value>(&json)?;
    let code: i32 = get_val!(value, "code")?;
    if code == 200 {
        let array: &Vec<Value> = get_val!(value, "banners")?;
        let mut vec: Vec<BannersInfo> = Vec::new();
        for v in array.iter() {
            let bi: BannersInfo = BannersInfo {
                pic: get_val!(v, "imageUrl")?,
                target_id: get_val!(v, "targetId")?,
                target_type: TargetType::from(get_val!(@as i32, v, "targetType")?),
                type_title: get_val!(v, "typeTitle").unwrap_or_default(),
            };
            vec.push(bi);
        }
        return Ok(vec);
    }
    Err(anyhow!("none"))
}

pub fn to_unikey(json: String) -> Result<String> {
    let value = &serde_json::from_str::<Value>(&json)?;
    let code: i32 = get_val!(value, "code")?;

    if code.eq(&200) {
        let unikey: String = get_val!(value, "unikey")?;
        return Ok(unikey);
    }
    Err(anyhow!("get unikey err!"))
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TopList {
    pub id: u64,
    pub name: String,
    pub update: String,
    pub description: String,
    pub cover: String,
}

pub fn to_toplist(json: String) -> Result<Vec<TopList>> {
    let value = &serde_json::from_str::<Value>(&json)?;
    let code: i32 = get_val!(value, "code")?;

    if code.eq(&200) {
        let list: &Vec<Value> = get_val!(value, "list")?;
        let mut toplist = Vec::new();
        for t in list {
            toplist.push(TopList {
                id: get_val!(t, "id")?,
                name: get_val!(t, "name")?,
                update: get_val!(t, "updateFrequency").unwrap_or_default(),
                description: get_val!(t, "description").unwrap_or_default(),
                cover: get_val!(t, "coverImgUrl").unwrap_or_default(),
            });
        }
        return Ok(toplist);
    }
    Err(anyhow!("get toplist err!"))
}

#[derive(Debug)]
pub enum Method {
    Post,
    Get,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum TargetType {
    Song,
    Album,
    Unknown,
}

impl From<i32> for TargetType {
    fn from(t: i32) -> Self {
        match t {
            1 => Self::Song,
            10 => Self::Album,
            _ => Self::Unknown,
        }
    }
}
