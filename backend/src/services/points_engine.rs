use crate::models::PlayerPosition;

/// Points calculation engine for MRR Fantasy.
///
/// Calculates total fantasy points for a player based on their
/// performance stats and position.
pub struct PointsEngine;

impl PointsEngine {
    /// Calculate total fantasy points for a player in a match week.
    ///
    /// # Points System
    /// - Goal (FWD): +10
    /// - Goal (MID): +8
    /// - Goal (DEF/GK): +12
    /// - Assist: +5
    /// - Clean Sheet (DEF/GK): +6
    /// - Save (GK): +2 per save
    /// - Tackle won: +2
    pub fn calculate(
        position: &PlayerPosition,
        goals: i32,
        assists: i32,
        clean_sheets: i32,
        saves: i32,
        tackles: i32,
    ) -> i32 {
        let goal_points = match position {
            PlayerPosition::Fwd => goals * 10,
            PlayerPosition::Mid => goals * 8,
            PlayerPosition::Def | PlayerPosition::Gk => goals * 12,
        };

        let assist_points = assists * 5;

        let clean_sheet_points = match position {
            PlayerPosition::Def | PlayerPosition::Gk => clean_sheets * 6,
            _ => 0,
        };

        let save_points = match position {
            PlayerPosition::Gk => saves * 2,
            _ => 0,
        };

        let tackle_points = tackles * 2;

        goal_points + assist_points + clean_sheet_points + save_points + tackle_points
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forward_goal_points() {
        let pts = PointsEngine::calculate(&PlayerPosition::Fwd, 2, 1, 0, 0, 0);
        // 2 goals * 10 + 1 assist * 5 = 25
        assert_eq!(pts, 25);
    }

    #[test]
    fn test_midfielder_goal_points() {
        let pts = PointsEngine::calculate(&PlayerPosition::Mid, 1, 2, 0, 0, 1);
        // 1 goal * 8 + 2 assists * 5 + 1 tackle * 2 = 20
        assert_eq!(pts, 20);
    }

    #[test]
    fn test_defender_clean_sheet() {
        let pts = PointsEngine::calculate(&PlayerPosition::Def, 0, 0, 1, 0, 3);
        // 0 goals + 0 assists + 1 cs * 6 + 3 tackles * 2 = 12
        assert_eq!(pts, 12);
    }

    #[test]
    fn test_goalkeeper_saves() {
        let pts = PointsEngine::calculate(&PlayerPosition::Gk, 0, 0, 1, 5, 0);
        // 0 goals + 0 assists + 1 cs * 6 + 5 saves * 2 = 16
        assert_eq!(pts, 16);
    }

    #[test]
    fn test_goalkeeper_heroic_goal() {
        let pts = PointsEngine::calculate(&PlayerPosition::Gk, 1, 0, 1, 3, 0);
        // 1 goal * 12 + 1 cs * 6 + 3 saves * 2 = 24
        assert_eq!(pts, 24);
    }

    #[test]
    fn test_zero_stats() {
        let pts = PointsEngine::calculate(&PlayerPosition::Fwd, 0, 0, 0, 0, 0);
        assert_eq!(pts, 0);
    }
}
