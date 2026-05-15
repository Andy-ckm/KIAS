#!/bin/bash
# KIAS GitHub 推送脚本
# 使用方法: 
#   1. 先将SSH公钥添加到GitHub (Settings → SSH and GPG keys)
#   2. 运行此脚本: ./push-to-github.sh

set -e

echo "=== KIAS GitHub 推送脚本 ==="
echo ""

# 检查SSH密钥
if [ ! -f ~/.ssh/id_ed25519_kias ]; then
    echo "❌ SSH密钥不存在，请先生成: ssh-keygen -t ed25519 -C 'kias-deploy' -f ~/.ssh/id_ed25519_kias"
    exit 1
fi

# 显示公钥
echo "📋 请将以下公钥添加到GitHub (Settings → SSH and GPG keys):"
echo "────────────────────────────────────────────────────────────"
cat ~/.ssh/id_ed25519_kias.pub
echo "────────────────────────────────────────────────────────────"
echo ""

# 等待用户确认
read -p "已添加公钥到GitHub？(y/N): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "请先添加公钥后再运行此脚本"
    exit 1
fi

# 配置远程仓库
echo "1. 配置远程仓库..."
git remote remove origin 2>/dev/null || true
git remote add origin git@github.com:Andy-ckm/KIAS.git

# 推送代码
echo "2. 推送代码到 GitHub..."
GIT_SSH_COMMAND="ssh -i ~/.ssh/id_ed25519_kias -o StrictHostKeyChecking=no" git push -u origin master

echo ""
echo "✅ 推送完成！"
echo "仓库地址: https://github.com/Andy-ckm/KIAS"
