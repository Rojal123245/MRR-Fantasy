use crate::models::PlayerPosition;

pub struct PointsEngine;

impl PointsEngine {
    /// Calculate total fantasy points for a player in a match week.
    ///
    /// Scoring per MRR Fantasy rules:
    /// - Goals: GK=10, DEF=6, MID=5, FWD=4
    /// - Assists: 5 pts each
    /// - Clean Sheets: GK=2, DEF=2
    /// - Saves (GK only): 1 pt per 5 saves
    /// - Penalty Save: +8
    /// - Minutes: 35+ = 2, 1-34 = 1
    /// - Regular Foul: -1
    /// - Serious Foul: -3
    /// - Own Goal: -2
    /// - Penalty Miss: -2
    pub fn calculate(
        position: &PlayerPosition,
        goals: i32,
        assists: i32,
        clean_sheets: i32,
        saves: i32,
        penalty_saves: i32,
        own_goals: i32,
        penalty_misses: i32,
        regular_fouls: i32,
        serious_fouls: i32,
        minutes_played: i32,
    ) -> i32 {
        let goal_pts = goals
            * match position {
                PlayerPosition::Gk => 10,
                PlayerPosition::Def => 6,
                PlayerPosition::Mid => 5,
                PlayerPosition::Fwd => 4,
            };

        let assist_pts = assists * 5;

        let cs_pts = clean_sheets
            * match position {
                PlayerPosition::Gk => 2,
                PlayerPosition::Def => 2,
                _ => 0,
            };

        let save_pts = match position {
            PlayerPosition::Gk => saves / 5,
            _ => 0,
        };

        let pen_save_pts = penalty_saves * 8;

        let minutes_pts = if minutes_played >= 35 {
            2
        } else if minutes_played >= 1 {
            1
        } else {
            0
        };

        let negative =
            own_goals * -2 + penalty_misses * -2 + regular_fouls * -1 + serious_fouls * -3;

        goal_pts + assist_pts + cs_pts + save_pts + pen_save_pts + minutes_pts + negative
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forward_goal_points() {
        let pts = PointsEngine::calculate(&PlayerPosition::Fwd, 2, 1, 0, 0, 0, 0, 0, 0, 0, 60);
        // 2 goals * 4 + 1 assist * 5 + 2 (minutes 60) = 15
        assert_eq!(pts, 15);
    }

    #[test]
    fn test_midfielder_goal_and_assist() {
        let pts = PointsEngine::calculate(&PlayerPosition::Mid, 1, 2, 0, 0, 0, 0, 0, 0, 0, 50);
        // 1 goal * 5 + 2 assists * 5 + 2 (minutes 50) = 17
        assert_eq!(pts, 17);
    }

    #[test]
    fn test_defender_clean_sheet() {
        let pts = PointsEngine::calculate(&PlayerPosition::Def, 0, 0, 1, 0, 0, 0, 0, 0, 0, 60);
        // 1 cs * 2 + 2 (minutes 60) = 4
        assert_eq!(pts, 4);
    }

    #[test]
    fn test_goalkeeper_saves() {
        let pts = PointsEngine::calculate(&PlayerPosition::Gk, 0, 0, 1, 10, 0, 0, 0, 0, 0, 60);
        // 1 cs * 2 + 10/5=2 saves + 2 (minutes 60) = 6
        assert_eq!(pts, 6);
    }

    #[test]
    fn test_goalkeeper_heroic_goal() {
        let pts = PointsEngine::calculate(&PlayerPosition::Gk, 1, 0, 1, 5, 1, 0, 0, 0, 0, 60);
        // 1 goal * 10 + 1 cs * 2 + 5/5=1 save + 1 pen_save * 8 + 2 (minutes) = 23
        assert_eq!(pts, 23);
    }

    #[test]
    fn test_zero_stats() {
        let pts = PointsEngine::calculate(&PlayerPosition::Fwd, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        assert_eq!(pts, 0);
    }

    #[test]
    fn test_negative_points() {
        let pts = PointsEngine::calculate(&PlayerPosition::Mid, 0, 0, 0, 0, 0, 1, 1, 2, 1, 60);
        // 0 goals + 0 assists + 1 OG * -2 + 1 pen_miss * -2 + 2 reg_fouls * -1 + 1 serious * -3 + 2 (mins)
        // = -2 + -2 + -2 + -3 + 2 = -7
        assert_eq!(pts, -7);
    }

    #[test]
    fn test_low_minutes() {
        let pts = PointsEngine::calculate(&PlayerPosition::Fwd, 1, 0, 0, 0, 0, 0, 0, 0, 0, 20);
        // 1 goal * 4 + 1 (minutes 20) = 5
        assert_eq!(pts, 5);
    }
}
