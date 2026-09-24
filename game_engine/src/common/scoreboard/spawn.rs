use bevy::prelude::*;

use crate::components::{LivesUi, ScoreboardUi};
use crate::config::{
    SCOREBOARD_FONT_SIZE, SCOREBOARD_TEXT_PADDING, SCORE_COLOR, TEXT_COLOR,
};

/// 画面左上に横並びで「Lives: N   Score: M」と並べる（Lives が Score の左側）。
/// 横並び（flex Row）のコンテナに、Lives → Score の順で子として置く。
pub fn spawn_scoreboard(commands: &mut Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: SCOREBOARD_TEXT_PADDING,
            left: SCOREBOARD_TEXT_PADDING,
            column_gap: Val::Px(20.0),
            ..default()
        },
        children![
            (
                Text::new("Lives: "),
                TextFont {
                    font_size: SCOREBOARD_FONT_SIZE,
                    ..default()
                },
                TextColor(TEXT_COLOR),
                LivesUi,
                children![(
                    TextSpan::default(),
                    TextFont {
                        font_size: SCOREBOARD_FONT_SIZE,
                        ..default()
                    },
                    TextColor(SCORE_COLOR),
                )],
            ),
            (
                Text::new("Score: "),
                TextFont {
                    font_size: SCOREBOARD_FONT_SIZE,
                    ..default()
                },
                TextColor(TEXT_COLOR),
                ScoreboardUi,
                children![(
                    TextSpan::default(),
                    TextFont {
                        font_size: SCOREBOARD_FONT_SIZE,
                        ..default()
                    },
                    TextColor(SCORE_COLOR),
                )],
            ),
        ],
    ));
}
