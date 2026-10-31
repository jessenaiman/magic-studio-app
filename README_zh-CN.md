# Magic Studio

Magic Studio 是 Jesse Naiman 的桌面创作应用：在同一个项目中规划分镜、制作动画、
设计歌词，并编排歌曲视频。原始作者、上游来源及许可信息见
[来源说明](SOURCE-ATTRIBUTION.md)和 [LICENSE](LICENSE)。

[English](README.md) · [项目仓库](https://github.com/jessenaiman/magic-studio-app)

## 四个创作区

| 创作区 | 用途 |
|---|---|
| **Storyboard（分镜）** | 导入歌曲及带时间的歌词，编辑 DeckCraft 分镜并审阅画面。 |
| **Animation（动画）** | 准备分层角色、安排动作，并使用 Blender 渲染流程。 |
| **Lyrics（歌词）** | 按歌曲时间设计文字、重点词及动态效果。 |
| **Studio（工作室）** | 在项目内编排、预览 HyperFrames 视频。 |

Fidget 用于项目对话，Doodle 用于画面创作。源媒体、时间信息、可编辑文档及生成记录
保留在项目中；项目媒体应存放在用户选定的制作目录，而非源代码仓库。

## 使用已安装的应用

```sh
magic
magic-cli --help
magic-cli commands
```

`magic /absolute/Song.magic` 打开指定项目。CLI 和 MCP 与桌面应用使用同一 Rust
命令注册表；具体语法、项目选择与 MCP 配置见
[Magic 技能](agent-harness/cli_anything/magic/skills/SKILL.md)。
启动已安装的应用不需要重新配置源代码、Python 制作工具或下载模型。

## Backstage：独立观察工作区

Backstage 是 Rust/Ratatui 终端应用。Workshop 观察已保存项目，MOPS 展示 PR 任务交接，
Mayhem 展示工作树与原生审计证据，Settings 展示已观察到的路径和团队文档。
它不会启动或恢复智能体会话。

[英文主页](README.md#backstage--the-observer)展示真实终端截图，并说明导入快照与
当前审计之间的区别。[Backstage 文档](apps/backstage/README.md)记录使用方式及实际限制。

## 安装、开发和制作工具

[唯一的源码安装指南](agent-harness/cli_anything/magic/README.md)说明如何构建和安装原生
应用。CLI-Anything 命令适配器并不能替代原生应用安装。

Python 工具、流水线 YAML、阶段技能及模式定义仍可复用。参阅
[制作指南](MAGIC-STUDIO.md)、[项目结构](PROJECT_CONTEXT.md)、
[智能体指南](AGENT_GUIDE.md)及[贡献规则](CONTRIBUTING.md)。
准备好的分镜仍是草稿，不代表画面已完成或已获创作批准。
