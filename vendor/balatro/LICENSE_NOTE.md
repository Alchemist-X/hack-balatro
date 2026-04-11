# Balatro Source — Research Authorization

## 授权说明

本目录下的内容（`vendor/balatro/`）包含 Balatro 1.0.1o 的原始游戏资源和解包的 Lua 源码，**仅用于科研目的**。

- **授权范围**：科研项目 `hack-balatro`（高保真 Balatro 模拟环境 + AI 智能体研究）
- **使用限制**：
  - ✅ 用于引擎验证、规则参考、审计对照
  - ✅ 用于生成 ruleset bundle 和训练数据
  - ✅ 在合作者之间共享以复现研究
  - ❌ 不得用于商业发行
  - ❌ 不得用于重新分发完整游戏
  - ❌ 不得移除或规避游戏的版权/付费验证

## 内容清单

```
vendor/balatro/steam-local/
├── manifest.json                      # 元数据 + SHA256 校验
├── original/
│   └── Balatro.love                   # 原始游戏包（53 MB）
└── extracted/                         # 47 个解包 Lua 文件（3.8 MB）
    ├── game.lua                       # 权威规则源
    ├── main.lua / blind.lua / card.lua / tag.lua
    ├── functions/                     # state_events, UI, common_events 等
    ├── engine/                        # controller, event, node, sprite 等
    └── localization/                  # 15 种语言
```

## 完整性校验

- `Balatro.love` SHA256: `48c7a0791796a969d2cd0891ebdc9922b2988eb5aaad8ad7a72775a02772e24e`
- 生成时间: `2026-03-27T14:34:27+00:00`
- 来源: 授权持有者本地 Steam 安装

## 版权

Balatro © LocalThunk / Playstack. 所有游戏内容、代码、美术资源版权归原作者所有。本项目仅在获得的研究授权范围内使用这些资源。

任何对本仓库的 fork、clone、下载使用者需自行确认其使用场景符合上述授权约束。
