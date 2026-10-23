# 输入、事件与后端接线审计

审计路径：终端输入 → 上下文路由 → App 状态 → 后端命令/事件 → UI 反馈。

## 快捷键上下文

| 上下文 | 路由规则 |
| --- | --- |
| 快捷键帮助 | `F1`、`?`（非 Shell）或 `Alt+H` 优先打开；打开后 `←/→`、`↑/↓`、`Tab` 翻页，`F1`/`Esc`/`Alt+H` 关闭。 |
| 内嵌 Shell | 普通按键发送给 PTY；`F1`/`Alt+H` 打开宿主帮助，`F4`/`Alt+S` 收起；Shell 专属组合键包括搜索、标签和 OSC8 链接。 |
| 输入框 | `Enter` 提交、`Esc` 取消；普通字符和粘贴进入输入值，`F1` 可查看帮助。 |
| 音乐库 | 方向键移动焦点，`Enter` 打开/播放，`Backspace` 返回；每日推荐、私人 FM、最近播放等首页入口始终存在。 |
| 主界面 | 播放、歌词、动画、布局和系统级快捷键；单字符播放快捷键不会响应 Ctrl/Alt 组合，避免组合键误触发。 |
| 退出 | 主界面和音乐库均可用 `q` 退出，`Esc` 在弹窗/帮助中优先关闭当前层，`Ctrl-C` 始终退出非 Shell 界面；Shell 内 `Ctrl-C` 发送给 Shell。 |

同一份快捷键目录同时驱动 F1 帮助面板和配置化 Shell 标签提示，避免 UI 文案与路由分离。

## 音乐库导航

| 首页入口 | Auth 命令 | 后端路由 | 返回方式 |
| --- | --- | --- | --- |
| 每日推荐 | `browse daily` | `library_api.daily` | 结果页返回首页 |
| 私人 FM | `browse radio` | `library_api.collection("radio")` | 结果页返回首页 |
| 最近播放 | `browse recent` | `offline.recent` | 结果页返回首页 |
| 我喜欢的音乐 | `browse liked` | 用户歌单中的 `specialType=5` | 结果页返回首页 |
| 排行榜/收藏专辑/收藏歌手 | `browse charts/albums/artists` | 用户收藏或榜单集合 | 分类列表返回首页 |
| 用户歌单 | `songs playlist:<id>:<offset>` | `GetPlaylistInfo` + 批量 `GetTrackDetail` | 结果页返回首页 |
| 歌曲搜索 | `search <query>` | 分面搜索，分页保留 `search:<query>` kind | 结果页返回发起搜索前的页面 |

歌曲列表只在 `collection_offset < collection_total` 时允许 `PageDown`。搜索 kind 含冒号时会从右向左解析 `kind:id:offset`，不会把搜索词误当歌单 ID。

## 播放与 IPC

| Rust 动作 | NDJSON 命令/事件 | Python 处理 |
| --- | --- | --- |
| 暂停/继续 | `pause` / `resume` | miniaudio device stop/start |
| Seek | `seek` | HTTP Range/解码器 seek，设备退出后再重开流 |
| 音量/静音 | `volume` / `mute` | ReplayGain 后处理；逻辑音量与静音状态分离 |
| 歌词偏移 | `lyric_offset` | 立即重算歌词游标 |
| 下一首预载 | `preload` 首包 + `start` | 提前完成元数据、封面、歌词及首个解码流 |
| 播放反馈 | `playback` | 位置、逻辑音量、静音和暂停状态 |
| 错误 | `error` | code/scope/retryable 进入通知和历史 |

动画选择器每帧读取实时音频特征，约每二至四拍生成一次新的场景签名；普通场景最少保持四拍，分形章节最少保持八拍，并每三个场景强制进入一次 Detailed 候选池。歌词句边界只能在最短保持期之后触发重选。`math_kernel!` 的 `dynamic` 参数直接进入编译后函数签名，因此 RMS、Bass、Centroid、Stereo Width 等数据会参与数学表达式本身，而不是只改变外层坐标。

## 本次修正的错位

- `Alt-F` 在音乐库中曾被普通 `f` 收藏动作截获；现作为全局歌词搜索，在 Shell 中仍有独立的 scrollback 搜索语义。
- 普通播放快捷键曾接受任意 Ctrl/Alt 组合；现仅在无 Ctrl/Alt 时生效。
- 无 `--config` 时 `Ctrl-R` 曾会重置运行时选项；现在明确报错。配置重载也会实际切换动画内核。
- 搜索非歌曲类型曾没有返回栈并污染首页；所有搜索先记录当前音乐库页面。
- 一次性列表在底部仍可触发 `PageDown`；现在严格按 offset/total 判断。
- 认证错误曾可能触发对播放链接的“重试”；现在只选择 playback/session 范围的可恢复错误。
- 动画模式重载、配置文件和 Shell 标签提示已改为使用同一配置状态。
- 快捷键帮助曾吞掉 `q`，导致用户以为程序无法退出；现在帮助页中 `q`/`Ctrl-C` 会直接退出，`Esc`/`F1` 只关闭帮助。
- 音乐库分支曾忽略 `q`，现在音乐库打开时 `q` 也会立即退出并关闭音频后端。
- 父进程异常消失时子后端曾可能继续播放；现在 Python 侧同时监听 stdin EOF 和父 PID，父进程断开后会立即结束音频设备。
- 普通 `d` 曾与音乐库“每日推荐”语义冲突并禁用当前动画，多次操作会逐步禁用几乎所有模式；禁用现改为 `Alt+D`，`Alt+Shift+D` 可重置，且剩余模式少于四个时会自动修复。
- 重叠窗口不再让整块上层窗口变成半透明；只有实际相交矩形应用 80% frosted glass，其余区域保持不透明，避免动画、边框和文字多套穿透。
- 封面矩形变化时会清除 CoverArt scratch、恢复完整显现强度并重建 Kitty 签名；自动化测试覆盖拖动、切换样式和终端 resize 后封面仍持续输出。
- Kitty 图片被 `d=I` 删除后曾继续保留“已上传”缓存，重新显示时只发 placement 命令导致空框；现在清理图片时同步失效 payload，下一次显示会重新上传。
