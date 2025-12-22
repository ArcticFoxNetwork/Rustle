<p align="center">
  <img src="assets/icons/icon_256.png" width="128" alt="Rustle">
</p>

<h1 align="center">Rustle</h1>

<p align="center">
  一个使用 Rust + iced 构建的现代音乐播放器，支持网易云音乐和本地音乐库
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-2024-orange?style=flat-square" alt="Rust">
  <img src="https://img.shields.io/badge/License-AGPL--3.0-blue?style=flat-square" alt="License">
  <img src="https://img.shields.io/github/v/release/ArcticFoxNetwork/Rustle?style=flat-square" alt="Release">
</p>

## ✨ 特性

### 🎧 音乐播放
- 支持网易云音乐在线播放（需登录）
- 本地音乐库导入与管理
- 多种音质选择（128k / 192k / 320k / 无损 / Hi-Res）
- 播放模式：顺序播放、列表循环、单曲循环、随机播放
- 歌曲预加载，无缝切换
- 支持格式：MP3, FLAC, WAV, OGG, AAC, ALAC 等
  - 注：部分 M4A/ALAC 文件可能不支持拖动进度条（底层解码库限制）

### 🎨 界面设计
- 深色/浅色主题切换
- Apple Music 风格的歌词页面
- GPU 加速的 SDF 歌词渲染引擎
- 基于封面的动态背景色提取
- 流畅的 Spring 物理动画
- 省电模式（禁用动画和特效）

### 🎼 歌词支持
- LRC：标准行级歌词
- YRC：网易云逐字歌词
- QRC：QQ音乐逐字歌词
- TTML：Apple Music 歌词格式
- ESLrc：Foobar2000 ESLyric 格式
- LYS：Lyricify Syllable 格式
- 翻译歌词、罗马音支持

### 🔊 音频处理
- 10 段均衡器 + 预设
- 实时频谱可视化
- 音量标准化
- 淡入淡出效果
- 多音频设备选择

### 🖥️ 系统集成
- 系统托盘（最小化到托盘）
- MPRIS 媒体控制（Linux）
- 全局快捷键
- 代理设置（HTTP/HTTPS/SOCKS5）

## 📦 安装

### 从 Release 下载

前往 [Releases](../../releases) 页面下载对应平台的安装包：

| 平台 | 格式 |
|------|------|
| Linux x86_64 | AppImage |
| macOS Intel | DMG |
| macOS Apple Silicon | DMG |
| Windows x86_64 | EXE |

### 从源码构建

```bash
# 克隆仓库
git clone https://github.com/ArcticFoxNetwork/Rustle
cd rustle

# 安装依赖 (Ubuntu/Debian)
sudo apt-get install -y \
    libssl-dev \
    libdbus-1-dev \
    libasound2-dev

# 构建
cargo build --release

# 运行
./target/release/rustle
```

## 🎮 快捷键

| 快捷键 | 功能 |
|--------|------|
| `Space` | 播放/暂停 |
| `Ctrl+N` | 下一首 |
| `Ctrl+P` | 上一首 |
| `Ctrl+↑` | 增加音量 |
| `Ctrl+↓` | 减少音量 |
| `Ctrl+M` | 静音 |
| `Ctrl+→` | 快进 |
| `Ctrl+←` | 快退 |
| `Ctrl+H` | 返回首页 |
| `Ctrl+K` | 搜索 |
| `Q` | 显示/隐藏队列 |
| `F11` | 全屏 |

媒体键（如果键盘支持）也可以控制播放。

## 🛠️ 技术栈

- **GUI**: [iced](https://github.com/iced-rs/iced) - 跨平台 Rust GUI 框架
- **音频**: [rodio](https://github.com/RustAudio/rodio) + [symphonia](https://github.com/pdeljanov/Symphonia)
- **数据库**: [SQLx](https://github.com/launchbadge/sqlx) + SQLite
- **GPU 渲染**: [wgpu](https://github.com/gfx-rs/wgpu) - 歌词 SDF 渲染
- **文本塑形**: [cosmic-text](https://github.com/pop-os/cosmic-text)

## 📄 License

AGPL-3.0 License

## 🙏 致谢

- [AMLL](https://github.com/Steve-xmh/applemusic-like-lyrics) - 歌词格式参考
- [NeteaseCloudMusicApi](https://github.com/Binaryify/NeteaseCloudMusicApi) - API 参考
