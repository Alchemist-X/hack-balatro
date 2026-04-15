# BalatroBot Windows 验证环境搭建

在 Windows 机器上搭建 Balatro 真实客户端验证链：
`Steam → Balatro → Lovely Injector → Steamodded → BalatroBot → TCP 12346`

## 前置条件

- Windows 10/11
- Steam 已安装并登录
- Balatro 已购买并安装（通过 Steam）
- Python 3.12+（Anaconda 或独立安装均可）

## 一键安装

从 Linux 开发机 SSH 到 Windows 机器执行：

```bash
# 设置目标机器（替换为你的用户名和IP）
export WIN_USER=your_username
export WIN_HOST=your_ip

# 运行安装脚本
bash scripts/windows_setup/install.sh $WIN_USER $WIN_HOST
```

或者在 Windows 本地执行：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\windows_setup\install.ps1
```

## 手动安装步骤

如果自动脚本不适用，按以下步骤手动操作。

### 1. 安装 Lovely Injector

```bash
# 下载 Lovely v0.9.0
curl -sL https://github.com/ethangreen-dev/lovely-injector/releases/download/v0.9.0/lovely-x86_64-pc-windows-msvc.zip -o lovely.zip

# 解压 version.dll 到 Balatro 安装目录
# 默认路径: C:\Program Files (x86)\Steam\steamapps\common\Balatro\
tar -xf lovely.zip
copy version.dll "C:\Program Files (x86)\Steam\steamapps\common\Balatro\"
```

### 2. 安装 Steamodded

```bash
# 下载 Steamodded v1.0.0-beta-1606b
curl -sL https://github.com/Steamodded/smods/archive/refs/tags/1.0.0-beta-1606b.zip -o smods.zip

# 解压到 Mods 目录的 Steamodded 子文件夹（关键！）
tar -xf smods.zip
xcopy /E /I smods-1.0.0-beta-1606b "%APPDATA%\Balatro\Mods\Steamodded"
```

> **重要**：Steamodded 必须放在 `Mods/Steamodded/` 子目录内，不能直接散在 `Mods/` 根目录。
> Lovely 要求每个 mod 的 patch 文件（`lovely/*.toml`）在独立的子文件夹中。

### 3. 安装 BalatroBot mod

```bash
# 下载 BalatroBot
curl -sL https://github.com/coder/balatrobot/archive/refs/heads/main.zip -o balatrobot.zip

# 解压，只取 mod 部分
tar -xf balatrobot.zip
xcopy /E /I balatrobot-main\mod "%APPDATA%\Balatro\Mods\balatrobot"
```

#### 修复依赖声明（必需）

BalatroBot 的 `balatrobot.json` 声明了 `"dependencies": ["Steamodded (>=1.*)"]`，
但 Steamodded beta 版本号（`1.0.0~BETA-xxx`）无法被此格式匹配，导致 mod 不加载。

修复方法：编辑 `%APPDATA%\Balatro\Mods\balatrobot\balatrobot.json`，
将 `"dependencies"` 改为空数组：

```json
"dependencies": []
```

### 4. 安装 uv（Python 包管理器）

```bash
pip install uv
# 安装 Python 3.13（balatrobot 需要）
uv python install 3.13
```

### 5. 验证目录结构

最终 Mods 目录应该是：

```
%APPDATA%\Balatro\Mods\
├── Steamodded/           ← Steamodded 框架
│   ├── lovely/           ← Lovely patch 文件（71 个 .toml）
│   ├── src/              ← Steamodded Lua 源码
│   ├── libs/             ← 依赖库
│   ├── assets/
│   ├── manifest.json
│   └── ...
└── balatrobot/           ← BalatroBot mod
    ├── balatrobot.json   ← manifest（dependencies 已清空）
    ├── balatrobot.lua    ← 入口
    └── src/lua/          ← API 源码
```

## 启动

1. **通过 Steam 启动 Balatro**（必须通过 Steam，直接运行 exe 会因 DRM 失败）
2. 等待看到 Steamodded 加载画面，然后进入主菜单
3. 验证 BalatroBot 是否工作：

```powershell
curl.exe -s -X POST http://127.0.0.1:12346 `
  -H "Content-Type: application/json" `
  -d '{"jsonrpc":"2.0","method":"health","id":1}'
```

期望返回：
```json
{"result":{"status":"ok"},"jsonrpc":"2.0","id":1}
```

## 注意事项

- `balatrobot serve` 命令**不适用于 Windows**，因为它直接运行 Balatro.exe 绕过了 Steam DRM
- BalatroBot 的 TCP server 运行在 Balatro 游戏进程内部（Lua mod），不是外部 Python 进程
- 环境变量 `BALATROBOT_PORT`、`BALATROBOT_FAST` 等需要在启动 Balatro **之前**设置（通过 Steam 启动选项或系统环境变量）
- Lovely Injector 的 `version.dll` 哈希：`ccfed59e4d245b7802c684fc86708e0a937f584d6e07d1ecc11e8eae22f9fc1a`

## 从 Linux 远程操作

```bash
# 检查 Balatro 是否运行
ssh user@host "tasklist /FI \"IMAGENAME eq Balatro.exe\""

# 检查 BalatroBot 端口
ssh user@host "netstat -an | findstr 12346"

# 调用 health API
ssh user@host "curl.exe -s -X POST http://127.0.0.1:12346 -H \"Content-Type: application/json\" -d \"{\\\"jsonrpc\\\":\\\"2.0\\\",\\\"method\\\":\\\"health\\\",\\\"id\\\":1}\""
```

## 踩坑记录

1. **Steamodded 目录层级**：必须在 `Mods/Steamodded/` 子目录，不能散在 `Mods/` 根目录
2. **BalatroBot 依赖声明**：`Steamodded (>=1.*)` 无法匹配 beta 版本号，需要清空 dependencies
3. **SSH 执行 PowerShell**：`Invoke-WebRequest` 和 `Expand-Archive` 在 SSH session 中会静默失败，用 `tar` 和 `xcopy` 替代
4. **GitHub API 限流**：Windows 机器直接下载 GitHub 资源可能被限流，从 Linux 下载后 SCP 传过去
5. **balatrobot serve 不可用**：直接启动 Balatro.exe 绕过 Steam DRM，`[API loaded no]` 后立即退出
