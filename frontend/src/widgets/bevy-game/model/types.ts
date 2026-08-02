export type BevyGameProps = {
  width?: number;
  height?: number;
  /**
   * 背景画像の URL。同一オリジンの相対パス（例: `/assets/backgrounds/sample_sunset.png`）でも、
   * 外部の絶対 URL（例: `https://cdn.example.com/bg/xxx.png`）でも指定できる。
   *
   * ここで渡した画像を React 側が fetch し、バイト列を WASM(Bevy) に引き渡す。これにより
   * ゲーム本体（Rust/WASM）は 1 つのビルドのまま、サービスごとに背景だけを差し替えられる。
   * 外部 URL を使う場合、配信元が CORS を許可している必要がある（ブラウザの fetch を通すため）。
   * 未指定の場合は Bevy 側にバンドルされたデフォルト背景が使われる。
   */
  background?: string;
  /**
   * 初期ブロック配置。各ブロックの中心座標を Bevy のワールド座標で指定する。
   * 座標系は「中心原点・y 上向き・1 単位 = 1px」で、アリーナは x∈[-450, 450],
   * y∈[-300, 300]（画面中央が原点、上が +y）。背景と同様に起動時に一度だけ
   * `window.__BREAKOUT_CONFIG__` 経由で WASM(Bevy) に渡す。
   * 未指定 / 空配列なら Bevy 側のデフォルト配置（アリーナ敷き詰め）にフォールバックする。
   */
  bricks?: Array<{ x: number; y: number }>;
  /**
   * ブロック共通のセルの大きさ（幅・高さ、px 相当）。`bricks` の有無に関わらず効く。
   * - `bricks` を渡した場合: その配置のセルサイズとして使う。
   * - `bricks` を渡さない場合: `background` + `brickImage` の両方があれば、2 画像の差分から
   *   自動配置するブロックの粒度として使う。それも無ければ、アリーナを敷き詰めるデフォルト
   *   配置の粒度として使う。
   * 未指定の場合は Bevy 側のデフォルトサイズ（50x30）になる。
   */
  cellSize?: { width: number; height: number };
  /**
   * ブロックの見た目に使う画像の URL。背景と同様に React 側が fetch し、バイト列を WASM(Bevy)
   * に渡す。盤面全体にこの画像を比率維持で貼ったとみなし、各ブロックはその絵のうち自分が覆う
   * 領域だけを切り出して表示する（全ブロックが揃うと 1 枚の絵になり、壊すと背景画像が覗く）。
   * 同一オリジンの相対パスでも外部の絶対 URL でも指定可能（外部 URL は配信元の CORS 許可が必要）。
   * 未指定 / fetch 失敗なら Bevy 側の単色ブロックにフォールバックする。
   */
  brickImage?: string;
  /**
   * ゲームクリア（全ブロック破壊）を Bevy(WASM) が検知したときに呼ばれる。
   * Bevy 側は `window.dispatchEvent(new CustomEvent("breakout:gameclear",
   * { detail: { result: "clear", score } }))` を投げるだけで、遷移（リロード/画面移動）は
   * React が担う。未指定の場合は既定で `window.location.reload()`（＝リロードして次ゲーム）を
   * 行う。遷移先を変えたい場合はこのコールバックで上書きする（例: 結果画面へ `location.href = ...`）。
   */
  onGameClear?: (detail: { result: string; score: number }) => void;
  /**
   * ゲームオーバー（ライフ 0）を Bevy(WASM) が検知したときに呼ばれる（`breakout:gameover`、
   * `detail.result === "gameover"`）。`onGameClear` と対称で、未指定の場合は既定で
   * `window.location.reload()`（＝リロードして最初から）を行う。ゲームオーバー専用の遷移に
   * したい場合はこのコールバックで上書きする。クリアとの区別は `detail.result` で行う。
   */
  onGameOver?: (detail: { result: string; score: number }) => void;
};
