//! スコア・残りライフの Resource と、それを表示する UI マーカー Component の定義。

use bevy::prelude::*;

// This resource tracks the game's score
#[derive(Resource, Deref, DerefMut)]
pub struct Score(pub usize);

#[derive(Component)]
pub struct ScoreboardUi;

// 残りライフを保持する Resource。ボールが DeathZone に触れるたびに 1 減る。
#[derive(Resource, Deref, DerefMut)]
pub struct Lives(pub usize);

#[derive(Component)]
pub struct LivesUi;
