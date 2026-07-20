## 概要

Selenity製Discord bot「うだまなみ」のフォーク、「うだまなさ」。
DMに送ったメッセージを指定チャンネルに送る代筆機能と、LLMに接続されたChat bot機能、そして各種のコマンドを実行する機能を持つ。

## fork specificity

The following agent-targeted documents are written for the upstream "udamanami" (followed by `main` branch) and may not be entirely true for this fork "udamanasa" (developed on `dev` branch).
There are several changes applied to "udamanasa" from "udamanami".
Use the docs as reference but not ground truth, which should be derived from the codebase.

## docs routing

以下のファイルは必要なときだけ読むこと。

[[docs/001_sketch.md]]: これからの改修方針とか機能拡張予定とか
[[docs/002_deploy.md]]: GCEへのデプロイ手順と日々の運用
[[docs/003_observability.md]]: アーキテクチャ全体像と、GCE/Cloudflare両側のログの見方（障害調査の起点）
