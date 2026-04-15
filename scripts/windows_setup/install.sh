#!/bin/bash
# BalatroBot Windows 验证环境一键安装脚本
# 用法: bash install.sh <windows_user> <windows_host>
# 例如: bash install.sh wangwei 10.1.100.27

set -e

WIN_USER="${1:?用法: bash install.sh <user> <host>}"
WIN_HOST="${2:?用法: bash install.sh <user> <host>}"
SSH_TARGET="${WIN_USER}@${WIN_HOST}"

LOVELY_VERSION="v0.9.0"
STEAMODDED_VERSION="1.0.0-beta-1606b"

TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

echo "=== BalatroBot 验证环境安装 ==="
echo "目标: ${SSH_TARGET}"
echo ""

# --- 0. 检查 SSH 连通性 ---
echo "[0/5] 检查 SSH 连接..."
ssh -o ConnectTimeout=5 "${SSH_TARGET}" "echo ok" > /dev/null 2>&1 || {
    echo "错误: 无法 SSH 到 ${SSH_TARGET}，请确认免密登录已配置"
    exit 1
}

# --- 检查 Balatro 是否已安装 ---
BALATRO_EXE='C:\Program Files (x86)\Steam\steamapps\common\Balatro\Balatro.exe'
ssh "${SSH_TARGET}" "if exist \"${BALATRO_EXE}\" (echo found) else (echo missing)" 2>/dev/null | grep -q found || {
    echo "错误: Balatro.exe 未找到，请先通过 Steam 安装 Balatro"
    exit 1
}
echo "  Balatro.exe 已确认"

# --- 1. Lovely Injector ---
echo "[1/5] 安装 Lovely Injector ${LOVELY_VERSION}..."
LOVELY_URL="https://github.com/ethangreen-dev/lovely-injector/releases/download/${LOVELY_VERSION}/lovely-x86_64-pc-windows-msvc.zip"
curl -sL "${LOVELY_URL}" -o "${TMPDIR}/lovely.zip"
unzip -o "${TMPDIR}/lovely.zip" -d "${TMPDIR}/lovely" > /dev/null
scp "${TMPDIR}/lovely/version.dll" "${SSH_TARGET}:\"C:/Program Files (x86)/Steam/steamapps/common/Balatro/version.dll\"" > /dev/null
echo "  version.dll 已安装"

# --- 2. Steamodded ---
echo "[2/5] 安装 Steamodded ${STEAMODDED_VERSION}..."
SMODS_URL="https://github.com/Steamodded/smods/archive/refs/tags/${STEAMODDED_VERSION}.zip"
curl -sL "${SMODS_URL}" -o "${TMPDIR}/smods.zip"
scp "${TMPDIR}/smods.zip" "${SSH_TARGET}:C:/Users/${WIN_USER}/smods.zip" > /dev/null
ssh "${SSH_TARGET}" "
    cd C:\\Users\\${WIN_USER} &&
    tar -xf smods.zip 2>nul &&
    mkdir \"C:\\Users\\${WIN_USER}\\AppData\\Roaming\\Balatro\\Mods\\Steamodded\" 2>nul
    xcopy /E /I /Y smods-${STEAMODDED_VERSION} \"C:\\Users\\${WIN_USER}\\AppData\\Roaming\\Balatro\\Mods\\Steamodded\" >nul &&
    rd /s /q smods-${STEAMODDED_VERSION} 2>nul &&
    del smods.zip 2>nul &&
    echo done
" 2>/dev/null | tail -1
echo "  Steamodded 已安装到 Mods/Steamodded/"

# --- 3. BalatroBot mod ---
echo "[3/5] 安装 BalatroBot mod..."
BB_URL="https://github.com/coder/balatrobot/archive/refs/heads/main.zip"
curl -sL "${BB_URL}" -o "${TMPDIR}/balatrobot.zip"
unzip -o "${TMPDIR}/balatrobot.zip" -d "${TMPDIR}" > /dev/null

# 找到 mod 目录（balatrobot-main/mod/ 或 balatrobot-main/ 下直接有 balatrobot.json）
BB_MOD_SRC="${TMPDIR}/balatrobot-main/mod"
if [ ! -d "${BB_MOD_SRC}" ]; then
    BB_MOD_SRC="${TMPDIR}/balatrobot-main"
fi

scp -r "${BB_MOD_SRC}" "${SSH_TARGET}:C:/Users/${WIN_USER}/balatrobot_tmp" > /dev/null 2>&1
ssh "${SSH_TARGET}" "
    mkdir \"C:\\Users\\${WIN_USER}\\AppData\\Roaming\\Balatro\\Mods\\balatrobot\" 2>nul
    xcopy /E /I /Y C:\\Users\\${WIN_USER}\\balatrobot_tmp \"C:\\Users\\${WIN_USER}\\AppData\\Roaming\\Balatro\\Mods\\balatrobot\" >nul &&
    rd /s /q C:\\Users\\${WIN_USER}\\balatrobot_tmp 2>nul &&
    echo done
" 2>/dev/null | tail -1

# 修复依赖声明
cat > "${TMPDIR}/balatrobot.json" << 'MANIFEST'
{
  "id": "balatrobot",
  "name": "BalatroBot",
  "author": ["S1M0N38", "stirby", "phughesion", "besteon", "giewev"],
  "description": "BalatroBot API opening balatro to bot.",
  "prefix": "BB",
  "main_file": "balatrobot.lua",
  "priority": 0,
  "badge_colour": "4CAF50",
  "badge_text_colour": "FFFFFF",
  "display_name": "BB",
  "version": "1.4.0",
  "dependencies": []
}
MANIFEST
scp "${TMPDIR}/balatrobot.json" "${SSH_TARGET}:\"C:/Users/${WIN_USER}/AppData/Roaming/Balatro/Mods/balatrobot/balatrobot.json\"" > /dev/null
echo "  BalatroBot 已安装（依赖声明已修复）"

# --- 4. uv + Python 3.13 ---
echo "[4/5] 安装 uv 和 Python 3.13..."
ssh "${SSH_TARGET}" "
    pip install -q uv 2>nul
    uv python install 3.13 2>nul
    echo done
" 2>/dev/null | tail -1
echo "  uv + Python 3.13 已安装"

# --- 5. 验证 ---
echo "[5/5] 验证安装..."
echo ""
ssh "${SSH_TARGET}" "
    echo [检查项] 结果
    echo -------- ----
    if exist \"C:\\Program Files (x86)\\Steam\\steamapps\\common\\Balatro\\version.dll\" (echo Lovely_Injector OK) else (echo Lovely_Injector MISSING)
    if exist \"C:\\Users\\${WIN_USER}\\AppData\\Roaming\\Balatro\\Mods\\Steamodded\\lovely\\core.toml\" (echo Steamodded OK) else (echo Steamodded MISSING)
    if exist \"C:\\Users\\${WIN_USER}\\AppData\\Roaming\\Balatro\\Mods\\balatrobot\\balatrobot.lua\" (echo BalatroBot OK) else (echo BalatroBot MISSING)
" 2>/dev/null

echo ""
echo "=== 安装完成 ==="
echo ""
echo "下一步："
echo "  1. 在 Windows 机器上通过 Steam 启动 Balatro"
echo "  2. 等待看到 Steamodded 加载画面"
echo "  3. 验证: ssh ${SSH_TARGET} \"curl.exe -s -X POST http://127.0.0.1:12346 -H \\\"Content-Type: application/json\\\" -d \\\"{\\\\\\\"jsonrpc\\\\\\\":\\\\\\\"2.0\\\\\\\",\\\\\\\"method\\\\\\\":\\\\\\\"health\\\\\\\",\\\\\\\"id\\\\\\\":1}\\\"\""
