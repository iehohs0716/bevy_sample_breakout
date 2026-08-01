export type Level = {
  id: string;
  title: string;
  author: string;
  thumbnailUrl: string;
  background: string;
  brickImage?: string;
  bricks: Array<{ x: number; y: number }>;
  cellSize: { width: number; height: number };
  redirectUrl?: string;
};
