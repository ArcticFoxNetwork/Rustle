<p align="center">
  <img src="assets/icons/icon_256.png" width="100" alt="Rustle">
</p>

<h1 align="center">Rustle</h1>

<p align="center">
  <b>纯 Rust 构建的高性能音乐播放器</b><br>
  支持网易云音乐和本地音乐库
</p>

<p align="center">
  <a href="../../releases"><img src="https://img.shields.io/github/v/release/ArcticFoxNetwork/Rustle?style=flat-square&color=blue" alt="Release"></a>
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-green?style=flat-square" alt="Platform">
  <img src="https://img.shields.io/badge/License-AGPL--3.0-orange?style=flat-square" alt="License">
</p>

<p align="center">
  <a href="#-安装">安装</a> •
  <a href="#-特性">特性</a> •
  <a href="#-快捷键">快捷键</a>
</p>

---

<p align="center">
  <img src="assets/screenshots/lyrics_page.png" width="90%" alt="歌词页面">
</p>

<details>
<summary><b>📸 更多截图</b></summary>
<br>
<p align="center">
  <img src="assets/screenshots/playlist.png" width="90%" alt="播放列表">
</p>
<p align="center">
  <img src="assets/screenshots/settings_page.png" width="90%" alt="设置页面">
</p>
</details>

---

## 🚀 为什么选择 Rustle

### 对比 Electron 应用

| | Rustle | Electron 播放器 |
|:--|:--:|:--:|
| **内存占用** | ~250MB | 500MB+ |
| **磁盘占用** | ~15MB | 150MB+ |
| **CPU 空闲时** | <1% | 3-5% |
| **启动速度** | 10ms | 2-5秒 |

### 真正的跨平台体验

| 平台 | 系统托盘 | 媒体控制 |
|:--|:--:|:--:|
| **Linux** | freedesktop 标准 | MPRIS D-Bus |
| **Windows** | 原生托盘 | 系统媒体控制 |
| **macOS** | 菜单栏图标 | 控制中心集成 |

### 更懂你的使用习惯

- **播放状态持久化** - 关闭应用后，下次打开自动恢复播放列表和进度
- **无缝预加载** - 提前加载下一首，切歌零等待

### GPU 加速歌词渲染

- **Apple Music 风格** - 逐字高亮、弹簧物理动画
- **SDF 文字渲染** - GPU 加速，任意缩放不模糊
- **多格式支持** - LRC / YRC / QRC / TTML / ESLrc

---

## 📦 安装

前往 [Releases](../../releases) 下载对应平台的安装包：

| 平台 | 格式 | 架构 |
|:----:|:----:|:----:|
| Windows | `.exe` | x86_64 |
| macOS | `.dmg` | Intel / Apple Silicon |
| Linux | `.AppImage` | x86_64 |

**Arch Linux (AUR)**

```bash
# 预编译版本
yay -S rustle-bin

# 从源码编译
yay -S rustle
```

<details>
<summary><b>从源码构建</b></summary>

```bash
# 克隆仓库
git clone https://github.com/ArcticFoxNetwork/Rustle
cd Rustle

# 安装依赖 (Ubuntu/Debian)
sudo apt-get install -y libssl-dev libdbus-1-dev libasound2-dev

# 构建并运行
cargo build --release
./target/release/rustle
```
</details>

---

## ✨ 特性

| 🎧 音乐播放 | 🎨 界面设计 |
|:--|:--|
| 网易云音乐在线播放 | 深色/浅色主题 |
| 本地音乐库管理 | Apple Music 风格歌词 |
| 多音质 (128k ~ Hi-Res) | GPU 加速 SDF 渲染 |
| 无缝预加载切换 | Spring 物理动画 |

| 🎼 歌词格式 | 🔊 音频处理 |
|:--|:--|
| LRC / YRC / QRC | 10 段均衡器 |
| TTML / ESLrc / LYS | 实时频谱可视化 |
| 翻译 + 罗马音 | 音量标准化 |

| 🖥️ 系统集成 |
|:--|
| 系统托盘 / MPRIS (Linux) / 全局快捷键 / 代理设置 |

---

## 🎮 快捷键

| 播放控制 | | 导航 | |
|:--|:--|:--|:--|
| `Space` | 播放/暂停 | `Ctrl+H` | 首页 |
| `Ctrl+N/P` | 下/上一首 | `Ctrl+K` | 搜索 |
| `Ctrl+→/←` | 快进/快退 | `Q` | 队列 |
| `Ctrl+↑/↓` | 音量 +/- | `F11` | 全屏 |

---

## 🛠️ 技术栈

[iced](https://github.com/iced-rs/iced) • [rodio](https://github.com/RustAudio/rodio) • [symphonia](https://github.com/pdeljanov/Symphonia) • [wgpu](https://github.com/gfx-rs/wgpu) • [SQLx](https://github.com/launchbadge/sqlx) • [cosmic-text](https://github.com/pop-os/cosmic-text)

---

## 📄 License

[AGPL-3.0](LICENSE)

## 🙏 致谢

- [AMLL](https://github.com/Steve-xmh/applemusic-like-lyrics) - 歌词格式参考
- [NeteaseCloudMusicApi](https://github.com/Binaryify/NeteaseCloudMusicApi) - API 参考
