# epsilonAnime

[![CI](https://github.com/silverenternal/epsilonAnime/actions/workflows/ci.yml/badge.svg)](https://github.com/silverenternal/epsilonAnime/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

终端里的网易云音乐 ASCII 可视化器。Rust 负责全屏终端渲染、数学动画和歌词叠加，Python 负责网易云数据、封面点阵、流式音频解码与律动分析。

> Unofficial personal project. It is not affiliated with NetEase Cloud Music.
> Use it only with media you are authorized to access, and follow the service's
> terms and local copyright law.

## 功能

- 输入 `https://music.163.com/song?id=...` 后自动抓取标题、歌手、高清封面、逐行歌词和流式音频。
- OpenCV 在 512×512 工作面提取亮度、局部对比度、Sobel 边缘与显著度，生成约 8.4 万个彩色粒子；Rust 用局部锐化、真彩半块与稀疏盲文自适应重建可识别封面。
- 封面默认使用彩色盲文点阵；`--cover-style braille|hybrid|halfblock` 可切换盲文、自适应混合与真彩半块，兼顾照片辨识度和个人审美。
- 封面面板按终端字符 2:1 宽高比适配，使用双线性采样、局部对比度和边缘增强；盲文模式仍保留每个单元的原始封面色。
- 默认封面宽度比例降至 0.30，把更多画幅留给动画、频谱、日志和歌词，不再让封面占据主视觉。
- 扫码登录网易云后可分页读取完整用户歌单；二维码使用 2×4 盲文点阵压缩，在 20 行高的终端中也不会被截断。音乐库首页固定提供每日推荐、私人 FM、最近播放、我喜欢的音乐、排行榜、收藏专辑和收藏歌手入口。歌单与歌曲列表支持方向键选择、焦点跟随滚动和 Enter 播放，登录 Cookie 以 `0600` 权限保存在 `target/netease-session.json`。
- 默认 `adaptive` 场景：对当前歌词、歌曲信息与 BPM 做 SHA-256，选择一个数学场景并实时生成，不会同时叠加多个场景。
- 底部状态栏始终显示当前数学模式（如 `MATH spherical3d`）和 `] 下一场`，可直接确认动画引擎已接入。
- 可选的 18 种数学模式：spectral、julia、superformula、metaballs、harmonics、complex-flow、newton、torus-knot、fourier、interference、reaction、attractor，以及 Lorenz、Möbius、球谐、3D 波面、Hopf 纤维和 Klein 瓶六种空间函数。
- 3D 曲线在 2×4 子像素场中连续连线、按深度控制亮度，并使用 glam 相机投影；旋转明暗由 `math_kernel!` 生成，不再只是离散单点。
- 保留 `cinematic` 自动轮换，以及固定预设 `prism`、`orbital`、`ink`、`bloom`。
- 另保留 plasma、fbm、Julia、超公式、Clifford、metaball、IFS、Gray-Scott、流场和 DSL 等实验动画预设。
- numpy 实时提取 RMS、bass/mid/treble、spectral flux 与 onset，驱动动画参数。
- 歌词按音频帧时钟同步，显示当前句、翻译与句内进度条。
- 网易云热门/最新评论会作为 BPM、onset 和歌词事件驱动的横向弹幕出现。
- 弹幕限制在封面右侧专用轨道内，最多 4 条，统一横向速度，不会铺满全屏或逐帧跳位。
- 紧凑可拖动 `SPECTRUM` 面板把 24 帧 × 16 段频谱历史渲染为 2D braille 热图；`--no-audio-scope` 可关闭。
- 可关闭 `LOGS` 面板增量显示 Rust、后端、登录与预取日志；`--no-logs` 可在启动时隐藏。
- `F4` / `Alt-S` 打开内嵌 PTY Shell，可执行日常命令；隐藏后 Shell 进程仍保留，`exit` 或 `Ctrl-D` 才会结束。
- 所有窗口（NOW PLAYING、COVER、SPECTRUM、LOGS、LYRICS、网易云歌单弹窗和 SHELL）都支持鼠标拖动及右下角 `◢` 缩放。
- 窗口彼此重叠时，上层表面以 0.8 不透明度合成到底层内容，并带稳定细颗粒磨砂纹理，避免完全遮住下方的动画与窗口层次。
- 播放支持进度拖动、±5 秒跳转、音量、静音、顺序/列表循环/单曲循环/随机；队列和历史使用原子写入的版本化状态保存，`cargo clean` 不会清除。
- HTTP Range 与解码器 seek 优先；下一首会在结束前预取元数据、封面、歌词并打开解码器，切歌时直接启动预热点，减少网络等待造成的空隙。
- ReplayGain + gain-peak 自动归一，支持按名称选择输出设备；无可用设备时自动切换到静默分析模式。
- 歌词支持全时间轴滚动/搜索、enhanced/YRC 字符级时间轴、日文罗马音、翻译开关和 ±10 秒 offset；字符高亮在句内连续推进。
- 音乐库支持歌曲/专辑/歌手/歌单/MV 分面搜索、详情、相关歌单、热歌/新歌/飙升榜、每日推荐、私人 FM、最近播放、我喜欢的音乐与收藏项。
- 搜索结果、浏览页、歌单分页和最近播放支持离线回退；缓存标记会在 UI 中显示，登出会清理离线索引和媒体缓存。
- 动画偏好使用稳定 mode id，不随数组重排失效；按 BPM、能量、风格族选择场景，并在低性能机器上排除高成本模式。
- 场景每约 2–4 拍根据实时 BPM、RMS、频谱、质心和立体声宽度重选，并每三个场景插入一次至少八拍的分形章节；生成宏内部的动态参数也直接读取音乐特征。
- crossfade 同时覆盖 intensity/depth/light；`Alt+D` 禁用/恢复、`Alt+Shift+D` 重置策略，剩余模式少于四个时会自动修复配置。
- Ghostty/Kitty 可通过 Kitty graphics 显示真像素封面，其他终端以及窗口重叠/弹窗状态自动回退盲文。
- Shell 支持多标签、滚轮回看/全文搜索、持久历史、宿主粘贴、Shift+鼠标选择复制、OSC8 链接和独立 `EPSILON_ANIME_SHELL_CWD`。
- 窗口支持边缘吸附、碰撞避让、拖拽预览、焦点、最小化列表恢复、层级调整和带版本布局持久化；刻意全量堆叠仍保留 0.8 毛玻璃。
- 高频渲染 scratch 按线程复用；终端 writer 使用有界背压，`--stats` 在退出时输出稳定的 JSON 指标，包括采样、编码、写入等待与脏瓦片。
- 支持中文/英文/日文界面标题、高对比度与色盲安全调色板；`Ctrl-R` 安全重载配置，核心热键可在 TOML 中修改。
- panic 会写入 `target/crash.log`；错误和通知带 scope/code/retryable 字段并落盘，`Alt-R` 只重试可恢复错误。
- 中日文歌词按 Unicode 显示宽度居中；日文汉字/假名使用双宽单元格，超长句严格裁剪在歌词面板内。
- 信息卡、封面和歌词都是浮动面板，可用鼠标左键拖动；`r` 一键恢复响应式布局。
- 运行中按 `F1`、`?` 或 `Alt-H` 可打开分页快捷键面板；主界面底部始终保留 `F1 help` 提示。
- 应用运行中可直接粘贴网易云歌曲链接热切换歌曲；旧音频进程组会被完整终止，新会话在后台启动。
- 脏瓦片矩阵渲染：只扫描发生变化的 8×3 瓦片，在瓦片内合并横向 run 并复用颜色命令。
- 任意源码文件不超过 600 行；`scripts/check_file_lengths.sh` 可校验。
- 字形 profile 受开源终端图像项目 [Chafa](https://github.com/hpjansson/chafa) 的符号分类与覆盖率思路启发，详见 [THIRD_PARTY.md](docs/THIRD_PARTY.md)。
- 场景设计参考了 2026 年 10 月活跃的 trix-effects、drift、terminal_graphics 与 LEGACY-PIXY，调研记录见 [ANIMATION_RESEARCH.md](docs/ANIMATION_RESEARCH.md)。

## 环境

- Rust 1.94+
- uv 0.12+
- Ghostty、Kitty、iTerm2、WezTerm 等支持 truecolor 的终端
- macOS/Linux；Python 由 uv 管理，固定为 CPython 3.12

首次运行：

```bash
cd py
uv sync
cd ..
./scripts/run.sh --demo
```

## 使用

```bash
./scripts/run.sh 'https://music.163.com/song?id=3347119767'
```

常用参数：

```bash
./scripts/run.sh
./scripts/run.sh URL --animation superformula --palette neon --fps 120
./scripts/run.sh URL --charset mixed
./scripts/run.sh URL --cover-style braille
./scripts/run.sh URL --cover-style hybrid
./scripts/run.sh URL --no-audio-scope
./scripts/run.sh URL --no-logs
./scripts/run.sh URL --cover-backend auto
./scripts/run.sh URL --config config.example.toml
./scripts/run.sh URL --stats
./scripts/run.sh --demo
./scripts/run.sh --list-animations
./scripts/run.sh --list-palettes
./scripts/run.sh --list-charsets
./scripts/run.sh --bench 120
```

不带 URL 启动后，直接把网易云歌曲链接粘贴进终端即可开始播放。也可以按 `i` 打开输入条，手动输入链接后按 Enter。运行中再次粘贴链接会停止旧歌曲并在后台加载新会话。

一次粘贴多条链接会立即播放第一条，并预取下一条；当前歌曲结束后自动切换。预取缓存位于 `target/netease-cache/`，音频直链超过安全时间会回退到重新获取。

运行中可用鼠标左键拖动任意分区面板，按 `r` 重置布局，按 `q`、`Esc` 或 `Ctrl-C` 退出；音乐库打开时 `q` 仍会直接退出。Shell 打开时这些按键会发送给 Shell，先用 `F4` 或 `Alt-S` 收起；终端会恢复 bracketed paste、鼠标捕获、raw mode、光标和主屏幕。

播放与音乐库快捷键：

- `F1`、`?` 或 `Alt-H`：打开分页快捷键面板；`←/→` 或 `Tab` 翻页，`F1`/`Esc` 关闭。
- `g`：打开网易云扫码登录二维码，用网易云音乐 App 扫描。
- `l`：打开音乐库首页和完整用户歌单；首行固定为每日推荐；`↑/↓` 选择，`Enter` 进入或播放，`Backspace` 返回，`Esc` 关闭面板。
- `Space`：暂停/继续；暂停时音频和函数动画时钟一起冻结。
- `←/→`：后退/前进 5 秒；也可以直接点击歌词面板内的进度条。
- `-` / `=`：音量减/加；`m`：静音/恢复；`p`：切换顺序、列表循环、单曲循环、随机。
- `b` / `n`：上一首/下一首。歌词面板底部也有 `PREV`、`PAUSE/PLAY`、`NEXT` 鼠标按钮。
- `Tab`：循环 balanced / gallery / instrument 三种布局；拖动任意窗口内部可移动，拖动右下角 `◢` 可缩放，`r` 恢复平衡布局。
- `c`：运行中循环 braille / hybrid / halfblock 封面渲染器。
- `t` / `o`：切换翻译/罗马音；`,` / `.`：歌词 offset 减/加 0.1 秒。
- `Alt-F`：搜索已加载歌词全时间轴。
- `[` / `]`：手动上一个/下一个数学场景；`f` 收藏、`Alt+D` 禁用/恢复、`Alt+Shift+D` 重置动画策略、`j` 锁定、`w/W` 调整权重。
- `F3` / `Alt-L`：显示或隐藏实时日志面板。
- `F4` / `Alt-S`：打开或收起内嵌 Shell；使用 `$SHELL` 和 vt100 终端状态，支持方向键、Ctrl 组合键和粘贴。
- `Ctrl-Tab`：切换窗口焦点；`Alt-M`：最小化/恢复焦点窗口；`Alt-Shift-M`：恢复下一个最小化窗口；`Ctrl-PageUp/PageDown`：调整窗口遮挡层级。
- `Alt-N`：切换通知历史；`Alt-R`：重试当前可重试错误；`Alt-X`：退出网易云登录。
- 歌单弹窗内：`s` 搜索、`d` 每日推荐、`h` 排行榜、`a` 收藏专辑、`r` 收藏歌手、`i` 我喜欢的音乐、`m` 私人 FM、`u` 最近播放，`PageDown` 加载下一页。
- 歌单内 `v` 打开详情/相关推荐，`f` 收藏当前歌曲；`Backspace` 沿嵌套导航返回。
- Shell 内滚轮回看历史，`Shift+拖动` 选择文字，松开后通过 OSC52 复制到系统剪贴板。
- Shell 内 `Alt-F` 搜索 scrollback，`Alt-[`/`Alt-]` 切标签，`Alt-T` 新建标签，`Alt-W` 关闭标签，`Alt-O` 打开发现在历史中的 OSC8 链接。
- `Alt-Y` 循环中文/英文/日文标题；指定 `--config` 时，`Ctrl-R` 从 TOML 重载配置且保留终端安全状态。
- 歌单播放会自动续播并预取下一首；打开歌单后仍可直接粘贴歌曲链接切换。

Shell 打开时，普通按键和大多数组合键会发送给 Shell；`F1`/`Alt-H` 快捷键面板和 `F4`/`Alt-S` 收起操作仍由宿主处理。

VIP/无损歌曲可先按 `g` 扫码登录，也可用环境变量提供现有 Cookie：

```bash
NETEASE_COOKIE='你的 MUSIC_U' ./scripts/run.sh URL --animation fbm-warp
```

免费歌曲通常可匿名获取 standard 音质。音频直链约 20 分钟过期，后端会在重新请求时刷新。

## 配置

复制 [config.example.toml](config.example.toml)，按需调整：

```toml
animation = "cinematic"
palette = "afterglow"
charset = "mixed"
fps = 120
cover_blend = 0.62
cover_style = "braille"
cover_backend = "auto"
show_logs = true
show_lyrics = true
show_meta = true
show_stats = false
cover_width_ratio = 0.30
locale = "zh-cn"
accessibility = "standard"

[hotkeys]
reload = "ctrl-r"
cycle_locale = "alt-y"
shell_next_tab = "alt-]"
shell_previous_tab = "alt-["
```

`accessibility` 可取 `standard`、`high-contrast`、`deuteranopia`、`tritanopia`。CLI 参数覆盖配置文件。动画与调色板规格位于 `specs/animations.toml` 和 `specs/palettes.toml`，修改后重新编译即可。

## 架构

```text
NetEase URL
    │
    ▼
Python backend (uv + pyncm)
    ├─ track detail / lyrics / stream URL
    ├─ Pillow + OpenCV → 512×512 RGB + ~84k particles
    ├─ pyncm QR login + user playlists / playlist tracks
    └─ miniaudio stream_any → playback + numpy rhythm analysis
             │ NDJSON (meta/cover/features/lyric/state/error)
             ▼
Rust renderer (crossterm + rayon)
    ├─ animation kernel f(x,y,t,audio)
    ├─ layer blend: animation + cover + lyric + meta
    ├─ dirty-tile cell diff → async terminal writer thread
    └─ session manager (generation-tagged events, process-group hot swap)
```

进程与事件协议：

- Rust 是主进程，拥有终端并 spawn idle 的 `uv run python -m backend.main`。
- Python 后端以 idle 模式预热；切歌时直接激活预热点，同时后台再准备下一个空闲进程。
- 封面 RGB 解码在独立 cover worker 中完成，不阻塞渲染主线程。
- Python 通过 Unix stream 把播放中的 PCM 发到 Rust；Rust 用 SIMD 窗口/RMS 缩减和 rustfft 计算 16 段频谱、onset 与 BPM。
- 实时特征还包括频谱质心、扩散、平坦度、rolloff、过零率、crest factor 和立体声宽度，并直接进入自适应数学场景。
- 多条链接进入播放队列，下一首会提前抓取元数据、歌词、音频 URL 和封面缓存。
- 运行中粘贴链接时，后台会话管理器会终止旧后端的整个进程组，并启动新 generation，主线程自动丢弃旧 generation 的迟到事件。
- 帧编码完成后交给有界异步 writer 线程；渲染不会被终端写入直接阻塞，同时保持 delta 帧顺序和一致性。
- Python 音频回调只把事件放入 `JsonEmitter` 队列，NDJSON 序列化和 stdout 写出由独立线程完成，避免管道阻塞反压音频线程。
- 浮动布局由独立 `Layout` 管理，面板拖动只更新矩形坐标；渲染时动画仍铺满背景。
- 重叠检测按实际绘制层级进行：只有上层窗口对下方内容执行 80% glass 合成，避免所有面板无条件变透明。
- 日志面板每 300ms 增量读取 backend/auth/prefetch 日志文件，并用有界环形缓冲避免长期运行吃内存。
- 内嵌 Shell 由 portable-pty 创建真实 PTY，vt100 解析 ANSI 状态并映射为现有 cell buffer；隐藏面板不会终止 Shell。
- 登录和歌单请求由独立 auth 子进程执行，二维码轮询不会阻塞音频会话或终端主循环。
- 播放控制通过当前后端 stdin 的 NDJSON 命令传递；暂停使用 miniaudio `PlaybackDevice.stop/start`，上下曲通过 generation 热切换。
- seek 先使用 HTTP `Content-Range`/解码器 seek，失败才回退到重开并跳帧；音量与 ReplayGain 在 PCM 回调输出前处理；播放状态每 0.5 秒结构化回报。
- 预取使用 latest-wins 调度，新目标会终止过时进程；本地缓存按 128 个文件/512MB 做 LRU 清理并删除无人引用的封面临时文件。
- `Buffer` 把终端划分为 8×3 脏瓦片：写入只标记变化瓦片，flush 时跳过静态区域，并把相邻变化合并成横向输出 run。
- 后端 stdout 只输出 NDJSON；日志写 `target/backend.log`。
- 主时钟来自音频解码/播放帧，避免 wall-clock 漂移。
- 后端退出或歌曲结束时，Rust 展示最后状态并自动恢复终端。
- GitHub Actions 在 Ubuntu/macOS/Windows 执行 fmt、clippy、Rust/Python 测试、依赖审计、文件长度检查和 release artifact；Ubuntu 额外生成 LCOV 覆盖率。

## 数学与元编程

动画统一为逐点数学函数 `f(x,y,t,audio) -> intensity`，然后同时映射到字符密度和调色板亮度。

代码生成分四层：

1. `build.rs` 从 TOML 规格生成 `math_tables.rs`：sin/cos LUT、调色板、动画变体表。
2. `ascii_ramp!` / `palette!` / `animations!` 宏生成字符斜坡、调色板和动画注册表。
3. `bind!` 宏生成音频特征到动画参数的钳制、归一化和映射。
4. `math_kernel!` proc-macro 解析字符串数学表达式，在宿主侧粗网格采样自动推导 min/max，生成特化、归一化后的逐像素内核；`dynamic` 参数会保留为运行期变量，直接接收实时 RMS、Bass、Centroid、Stereo Width 等音乐特征。
5. `specs/spectral_modes.toml` 经 build.rs 生成二维谱模式表，`spectral_engine!` 宏展开为频谱合成函数。
6. `math_kernel!` 通过包装层桥接 `puruspe`（Bessel/gamma/erf）、`sphrs`（实球谐）和 `glam`（3D 旋转/投影），生成代码直接调用成熟库。3D 波面的基础波形同样由 DSL 宏生成后传入空间场。

生成时钟按 BPM 计算，每拍切成 8 个采样步（32 分音符），频谱系数只在采样步更新。Adaptive 会持续计算由 BPM、响度、频谱能量、质心、平坦度、立体声宽度和当前歌词组成的场景签名，每隔约四拍或乐句边界重新选择一次场景；生成宏内部的参数也持续接收实时音乐特征。轨迹衰减统一按 `dt` 计算，不随帧率改变速度。

注意：stable Rust 的 `const fn` 不能调用 `f32::sin`，所以数学 LUT 在 `build.rs` 或 proc-macro 宿主侧生成。

## 字形与亮暗

`--charset` 提供五套 profile：

- `ascii`：最保守的纯 ASCII 字符，适合字体受限的终端。
- `blocks`：`░▒▓` 与 `▁▂▃▄▅▆▇█`，明暗过渡细腻。
- `quadrants`：象限块元素，适合几何/低分辨率风格。
- `braille`：按点阵密度动态生成盲文字形，纹理更细。
- `mixed`：默认；混合 ASCII、shade、block 与 quadrant，视觉层次最丰富。

动画亮度经过全画面 90% 分位自动曝光、vignette、黑位/白位 stretch、gamma tone map 和 4×4 Bayer 有序抖动，再按字形覆盖率选择字符；高光会向白色或当前 palette 峰值少量 bloom。封面粒子和背景动画共享曝光，颜色仍保持平滑插值，抖动只作用于字形密度，避免出现彩色噪点。

文字使用 sRGB 相对亮度计算对比度：标题、当前歌词、翻译和状态信息会根据面板背景自动选择可读前景，低对比度颜色会被替换为白色或黑色。面板阴影、亮色边框和拖动高亮进一步强化层级。

## 性能

`./scripts/run.sh --bench 120` 在 160×48 网格上测所有内核。开发机 release 构建实测：

```text
Plasma       ~24000 fps   FbmWarp      ~112 fps
Waves        ~10500 fps   Julia        ~5900 fps
Superformula  ~4500 fps   Metaballs    ~5400 fps
Clifford/IFS ~14000+ fps  FlowField    ~1150 fps
GrayScott     ~7600 fps   DslPlasma   ~15000 fps
Adaptive 18 modes  ~485–16800 fps
Cinematic/Prism/Ink ~720–745 fps
Orbital      ~3560 fps    Bloom       ~4600 fps
```

默认目标为 120 FPS，运行时下限为 90 FPS。优化后的 `cargo run` dev profile 使用 `opt-level=2`，120×40、2×4 子像素实测约 104 FPS、单帧约 3.8ms；低成本帧会同时降低数学采样密度和着色密度，而不只是重复采样结果。

`--stats` 会在左上角显示实时 FPS、脏瓦片、采样/编码/写入耗时；退出时向 stderr 输出带 `schema: 1` 的稳定 JSON 指标。

## 测试

```bash
cargo test
(cd py && uv run python -m unittest discover -s tests -v)
./scripts/check_file_lengths.sh
```

当前验证包含 100 个 Rust 单元/属性测试和 33 个 Python 测试。属性测试覆盖畸形 NDJSON、任意 ANSI 单元、PCM 帧、终端按键与随机歌词文本。完整输入/事件接线见 [WIRING_AUDIT.md](docs/WIRING_AUDIT.md)。

无音频设备时，可单独验证后端解码与分析：

```bash
cd py
uv run python -m backend.main 'https://music.163.com/song?id=3347119767' \
  --analyze-only --max-seconds 5
```

## 故障排查

- `audio playback failed`：检查系统输出设备和音量；后端会优先刷新过期直链并降级到兼容 MP3。日志在 `target/backend.log`。
- `460 Cheating`：后端默认带 `X-Real-IP`，也可用 `NETEASE_REAL_IP` 覆盖。
- 封面或歌词为空：歌曲可能受版权限制；尝试 Cookie 或换免费歌曲。
- 看不到颜色：程序会主动关闭 `NO_COLOR` 影响，因为颜色是可视化的核心输出。
- 字符错位：换用覆盖 `▀` 和 CJK 字形的等宽字体。

仅供个人学习与可视化实验，请遵守网易云音乐服务条款及当地版权法规。

## License and privacy

Project source is available under the [MIT License](LICENSE). Third-party
components retain their original licenses; see [THIRD_PARTY.md](docs/THIRD_PARTY.md).

Runtime state and credentials are stored outside the repository. Do not publish
`target/`, Python virtual environments, backend logs, cookies, or local cache
files. Security reporting instructions are in [SECURITY.md](SECURITY.md).
