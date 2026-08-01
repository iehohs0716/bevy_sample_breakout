export type ImageBytes = {
  bytes: Uint8Array;
  mime?: string;
};

/**
 * 画像 URL を fetch してバイト列化する。背景画像・ブロック画像で共通の処理。
 * fetch に失敗した場合は undefined を返す（呼び出し側で Bevy のデフォルト表示に
 * フォールバックさせる。致命ではないため warn に留める）。
 */
export async function fetchImageBytes(
  url: string,
  label: string,
): Promise<ImageBytes | undefined> {
  try {
    const res = await fetch(url);
    if (!res.ok) {
      throw new Error(`${label}の取得に失敗: ${res.status} ${res.statusText}`);
    }
    const buf = await res.arrayBuffer();
    return {
      bytes: new Uint8Array(buf),
      // フォーマット判定用の MIME（例: "image/png"）。取得できなければ Bevy 側は png とみなす。
      mime: res.headers.get("content-type") ?? undefined,
    };
  } catch (error) {
    console.warn(`${label}の取得に失敗しました。デフォルトにフォールバックします。`, error);
    return undefined;
  }
}
