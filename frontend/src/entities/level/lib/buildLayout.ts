// 上に頂点を持つピラミッド（最上段 1 個 → 下段ほど増える）を中央揃えで生成する。
export function buildPyramidLayout(
  cellSize: { width: number; height: number }
): Array<{ x: number; y: number }> {
  const rows = 6;
  const colSpacing = cellSize.width; //セル幅のみ
  const rowSpacing = cellSize.height; // 同じくセル幅のみ
  const topY = 200; // 最上段の y（アリーナ上部）
  const bricks: Array<{ x: number; y: number }> = [];
  for (let row = 0; row < rows; row++) {
    const count = row + 1; // その段のブロック数
    const y = topY - row * rowSpacing;
    for (let col = 0; col < count; col++) {
      // 段の中央に対して左右対称に配置
      const x = (col - (count - 1) / 2) * colSpacing;
      bricks.push({ x, y });
    }
  }
  return bricks;
}

// 3行5列の長方形ブロックを中央揃えで生成する。
export function buildCenteredBlockLayout(
  cellSize: { width: number; height: number }
): Array<{ x: number; y: number }> {
  const rowCount = 3;
  const colCount = 5;
  const colSpacing = cellSize.width;
  const rowSpacing = cellSize.height;
  const topY = 150; // 最上段の y
  const bricks: Array<{ x: number; y: number }> = [];
  for (let row = 0; row < rowCount; row++) {
    const y = topY - row * rowSpacing;
    for (let col = 0; col < colCount; col++) {
      // 各行の列は中央揃え
      const x = (col - (colCount - 1) / 2) * colSpacing;
      bricks.push({ x, y });
    }
  }
  return bricks;
}
