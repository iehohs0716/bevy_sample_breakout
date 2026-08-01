#!/bin/bash

export LANG=ja_JP.UTF-8
export LC_ALL=ja_JP.UTF-8

set -euo pipefail

# Supabase (db + rest) をローカルで起動し、poc_check テーブルへの
# INSERT / SELECT 疎通確認を一気通貫で実行するスクリプト。
#
# poc_check テーブル自体は supabase-local/volumes/db/poc_check.sql が
# db コンテナの初回起動時に自動作成する（docker-compose.yml の db サービス参照）。
# 疎通確認の実体は check_connectivity.py（PostgREST への実リクエスト送信）。
#
# Usage:
#   ./run.sh

check_command() {
  if ! command -v "$1" &> /dev/null; then
    echo "Error: $1 コマンドが見つかりません。"
    echo "$2"
    exit 1
  fi
}

check_command "docker" "Docker Desktop 等をインストールしてください: https://www.docker.com/"
check_command "uv" "uvをインストールしてください: curl -LsSf https://astral.sh/uv/install.sh | sh"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# CLAUDE.md が置かれているディレクトリをリポジトリルートとする
# （check_connectivity.py の find_repo_root と同じ探索方法）
REPO_ROOT="$SCRIPT_DIR"
while [ "$REPO_ROOT" != "/" ] && [ ! -f "$REPO_ROOT/CLAUDE.md" ]; do
  REPO_ROOT="$(dirname "$REPO_ROOT")"
done
if [ ! -f "$REPO_ROOT/CLAUDE.md" ]; then
  echo "Error: CLAUDE.md が見つからず、リポジトリルートを特定できません。"
  exit 1
fi

if [ ! -f "$REPO_ROOT/.env" ]; then
  echo "Error: $REPO_ROOT/.env が見つかりません。"
  echo "$REPO_ROOT/.env.example をコピーして .env を作成してください。"
  exit 1
fi

echo "========================================="
echo "Supabase (db + rest) を起動します"
echo "========================================="
(cd "$REPO_ROOT" && docker compose up -d db rest)

echo ""
echo "rest (PostgREST) が healthy になるまで待機します..."
CONTAINER_NAME="supabase-rest"
health="unknown"
for _ in $(seq 1 30); do
  health="$(docker inspect --format='{{.State.Health.Status}}' "$CONTAINER_NAME" 2>/dev/null || echo "unknown")"
  if [ "$health" = "healthy" ]; then
    break
  fi
  sleep 2
done

if [ "$health" != "healthy" ]; then
  echo "Error: rest (PostgREST) が healthy になりませんでした（最終状態: $health）。"
  echo "  docker compose logs rest で確認してください。"
  exit 1
fi
echo "rest は healthy です。"

echo ""
echo "========================================="
echo "poc_check テーブルへ INSERT / SELECT の疎通確認"
echo "========================================="
"$SCRIPT_DIR/check_connectivity.py" insert --label "run.sh による自動確認"
"$SCRIPT_DIR/check_connectivity.py" select

echo ""
echo "========================================="
echo "疎通確認が完了しました"
echo "========================================="
echo "スタックを停止する場合: (cd \"$REPO_ROOT\" && docker compose down)"
