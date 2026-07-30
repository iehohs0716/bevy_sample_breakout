#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "httpx>=0.27.0",
# ]
# ///

"""
Supabase REST API (PostgREST) への疎通確認クライアント。

supabase-local/ で docker compose 起動している db + rest (PostgREST) に対し、
Supabase の REST API 経由（anon key のみ、Supabase Auth/GoTrue は使わない）で
poc_check テーブルへの INSERT / SELECT を行い、書き込みが実際に反映されるかを確認する。

使い方:
    ./check_connectivity.py insert --label "手動確認1"
    ./check_connectivity.py select
"""

import argparse
import logging
import sys
from pathlib import Path

import httpx

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[logging.StreamHandler(sys.stdout)],
)
logger = logging.getLogger(__name__)

DEFAULT_REST_URL = "http://localhost:8001"


def find_repo_root(start: Path) -> Path:
    """
    CLAUDE.md が置かれているディレクトリをリポジトリルートとして遡って探す。

    このスクリプト自体の置き場所（sandbox/ 配下等）が変わっても .env の位置を
    固定の相対階層数（parent.parent 等）に頼らず解決できるようにするため。
    """
    for candidate in [start, *start.parents]:
        if (candidate / "CLAUDE.md").exists():
            return candidate
    raise FileNotFoundError(f"CLAUDE.md が見つからず、リポジトリルートを特定できません: {start}")


def default_env_file() -> Path:
    repo_root = find_repo_root(Path(__file__).resolve().parent)
    return repo_root / ".env"


def load_anon_key(env_file: Path) -> str:
    """
    supabase-local/.env から ANON_KEY を読む。

    ローカル専用のデモ用シークレットとはいえ、このスクリプト自身にはハードコードせず、
    設定ファイル側に一箇所だけ書かれている値を都度読みに行く。
    """
    if not env_file.exists():
        raise FileNotFoundError(f"env file not found: {env_file}")
    for line in env_file.read_text().splitlines():
        if line.startswith("ANON_KEY="):
            return line.split("=", 1)[1].strip()
    raise ValueError(f"ANON_KEY not found in {env_file}")


def insert_row(rest_url: str, anon_key: str, label: str) -> dict:
    """poc_check テーブルへ1行 INSERT し、PostgREST が返した行を返す。"""
    headers = {
        "apikey": anon_key,
        "Authorization": f"Bearer {anon_key}",
        "Content-Type": "application/json",
        "Prefer": "return=representation",
    }
    logger.info(f"POST {rest_url}/poc_check label={label!r}")
    response = httpx.post(f"{rest_url}/poc_check", headers=headers, json={"label": label})
    response.raise_for_status()
    rows = response.json()
    logger.info(f"inserted: {rows}")
    return rows[0]


def select_rows(rest_url: str, anon_key: str) -> list[dict]:
    """poc_check テーブルを全件 SELECT する。"""
    headers = {"apikey": anon_key, "Authorization": f"Bearer {anon_key}"}
    logger.info(f"GET {rest_url}/poc_check?select=*")
    response = httpx.get(f"{rest_url}/poc_check", headers=headers, params={"select": "*"})
    response.raise_for_status()
    rows = response.json()
    logger.info(f"{len(rows)} 件取得")
    return rows


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Supabase REST API (PostgREST) への疎通確認クライアント",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--rest-url",
        default=DEFAULT_REST_URL,
        help=f"PostgREST の URL (既定: {DEFAULT_REST_URL})",
    )
    parser.add_argument(
        "--env-file",
        type=Path,
        default=None,
        help="ANON_KEY を読む .env のパス (既定: リポジトリルート直下の .env を自動探索)",
    )

    subparsers = parser.add_subparsers(dest="command", required=True)

    insert_parser = subparsers.add_parser("insert", help="poc_check テーブルに1行 INSERT する")
    insert_parser.add_argument("--label", required=True, help="挿入する label 値")

    subparsers.add_parser("select", help="poc_check テーブルを全件 SELECT する")

    args = parser.parse_args()

    try:
        env_file = args.env_file if args.env_file is not None else default_env_file()
        anon_key = load_anon_key(env_file)
    except (FileNotFoundError, ValueError) as e:
        logger.error(str(e))
        sys.exit(1)

    try:
        if args.command == "insert":
            insert_row(args.rest_url, anon_key, args.label)
        else:
            rows = select_rows(args.rest_url, anon_key)
            for row in rows:
                print(row)
    except httpx.HTTPStatusError as e:
        logger.error(f"HTTPエラー: {e.response.status_code} {e.response.text}")
        sys.exit(1)
    except httpx.ConnectError as e:
        logger.error(f"接続エラー: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
