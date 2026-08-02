//! `Update` スケジュールで実行される system（毎フレーム／衝突イベントへの反応）への入り口。
//! ボールと他エンティティの当たり判定は子モジュール `collision` に、ブロック単一ドメインの
//! system・observer は子モジュール `brick` に分離している。

mod collision;
pub use collision::{
    check_ball_brick_collision, check_ball_deathzone_collision, check_ball_paddle_collision,
    check_ball_wall_collision,
};

mod brick;
pub use brick::{
    check_game_clear, draw_brick_outlines, mark_broken_edges_on_brick_destroyed,
    redraw_broken_bricks,
};
