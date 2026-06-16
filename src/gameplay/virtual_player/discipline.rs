//! Match-clock-driven AI discipline: how the round clock and the capture
//! scoreline reshape what a team's cars commit to in the closing stretch.
//!
//! The pure policy half of the virtual-player brain, split out from the
//! [`drive`](super::drive) system that applies it: where the drive system owns
//! the ECS mechanics (querying cars, choosing and applying targets), this module
//! owns the legible *decision* a team makes as the clock runs down. Given only the
//! [`MatchClock`] and the [`CaptureScore`] it answers three complementary
//! questions per team:
//!
//! - [`objective_commitment`]: a side *not ahead* drops cash detours to race the
//!   flag (a trailing team, and both sides of a level sudden death, chase the
//!   decider; a closing-time leader keeps playing the field).
//! - [`lead_protection`]: a side *strictly ahead* recalls a car to guard its lead.
//! - [`closing_time_pickup_discipline`]: the per-team union of the two, since a
//!   committing trailer and a protecting leader both leave cash bags on the track
//!   once the clock is running out.
//!
//! Outside the closing stretch, and for an unstarted match (no clock), every
//! answer is "no": the field plays on as normal.

use crate::gameplay::ctf::{CaptureScore, MatchClock};
use crate::gameplay::virtual_player::ai::AiTeam;

/// Per-team flag for closing-time clutch play: should this team's cars commit to
/// the CTF objective and stop chasing opportunistic pickup detours?
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ObjectiveCommitment {
    blue: bool,
    red: bool,
}

impl ObjectiveCommitment {
    pub const fn for_team(self, team: AiTeam) -> bool {
        match team {
            AiTeam::Blue => self.blue,
            AiTeam::Red => self.red,
        }
    }
}

/// Decides which teams commit to the objective given the round clock and the
/// capture scoreline.
///
/// Outside the closing stretch no team commits. Within it every team that is not
/// ahead on captures does: a closing-time leader keeps playing the field, while a
/// trailing side, and both sides of a level sudden death, drop cash detours to
/// race the flag. A missing clock (an unstarted match) never forces commitment.
pub fn objective_commitment(
    clock: Option<&MatchClock>,
    captures: CaptureScore,
) -> ObjectiveCommitment {
    if !clock.is_some_and(|clock| clock.is_closing_time()) {
        return ObjectiveCommitment::default();
    }
    ObjectiveCommitment {
        blue: captures.player <= captures.opponents,
        red: captures.opponents <= captures.player,
    }
}

/// Per-team flag for closing-time lead protection: should this team, ahead on
/// captures with the clock running down, recall a car to guard its lead?
///
/// The exact complement of [`ObjectiveCommitment`]: in the closing stretch every
/// team is either committing to attack (not ahead) or protecting a lead (ahead),
/// so the trailing/level side races the flag while the leading side digs in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LeadProtection {
    blue: bool,
    red: bool,
}

impl LeadProtection {
    pub const fn for_team(self, team: AiTeam) -> bool {
        match team {
            AiTeam::Blue => self.blue,
            AiTeam::Red => self.red,
        }
    }
}

/// Decides which teams protect a lead given the round clock and the scoreline.
///
/// Outside the closing stretch no team protects. Within it a team strictly ahead
/// on captures recalls a car to guard (see
/// [`crate::gameplay::virtual_player::ai::lead_defence_car`]); a trailing or
/// level side does not, since it is busy committing to the objective. A missing
/// clock (an unstarted match) never forces protection. Mirrors and complements
/// [`objective_commitment`]: a team protects exactly when it is *not* committing.
pub fn lead_protection(clock: Option<&MatchClock>, captures: CaptureScore) -> LeadProtection {
    if !clock.is_some_and(|clock| clock.is_closing_time()) {
        return LeadProtection::default();
    }
    LeadProtection {
        blue: captures.player > captures.opponents,
        red: captures.opponents > captures.player,
    }
}

/// Per-team flag for closing-time pickup discipline: should this team's cars
/// leave cash bags on the track and only break off an objective for a real edge?
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClosingTimePickupDiscipline {
    blue: bool,
    red: bool,
}

impl ClosingTimePickupDiscipline {
    pub const fn for_team(self, team: AiTeam) -> bool {
        match team {
            AiTeam::Blue => self.blue,
            AiTeam::Red => self.red,
        }
    }
}

/// Decides which teams discipline their pickup detours given the round clock and
/// scoreline.
///
/// In the closing stretch every team is either committing to attack (not ahead,
/// see [`objective_commitment`]) or protecting a lead (ahead, see
/// [`lead_protection`]); with the clock running out a cash bag is a distraction
/// either way, so the discipline is the union of the two complementary
/// predicates and a closing-time leader stops farming cash just as a trailing
/// team does. Outside the closing stretch neither holds, so no team disciplines
/// and cash bags are fair game again.
pub fn closing_time_pickup_discipline(
    clock: Option<&MatchClock>,
    captures: CaptureScore,
) -> ClosingTimePickupDiscipline {
    let commit = objective_commitment(clock, captures);
    let protect = lead_protection(clock, captures);
    ClosingTimePickupDiscipline {
        blue: commit.for_team(AiTeam::Blue) || protect.for_team(AiTeam::Blue),
        red: commit.for_team(AiTeam::Red) || protect.for_team(AiTeam::Red),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_team_commits_to_the_objective_outside_closing_time() {
        use crate::gameplay::ctf::{MatchPhase, CLOSING_TIME_FRAMES};

        let clock = MatchClock {
            frames_remaining: CLOSING_TIME_FRAMES + 1,
            phase: MatchPhase::Regulation,
        };
        assert_eq!(
            objective_commitment(
                Some(&clock),
                CaptureScore {
                    player: 0,
                    opponents: 2,
                },
            ),
            ObjectiveCommitment::default(),
            "a round with time to spare must not force any team to commit"
        );
    }

    #[test]
    fn closing_time_commits_every_team_that_is_not_ahead() {
        use crate::gameplay::ctf::{MatchPhase, CLOSING_TIME_FRAMES};

        let clock = MatchClock {
            frames_remaining: CLOSING_TIME_FRAMES,
            phase: MatchPhase::Regulation,
        };
        // Blue (player) trails Red (opponents) on captures.
        let commitment = objective_commitment(
            Some(&clock),
            CaptureScore {
                player: 1,
                opponents: 2,
            },
        );
        assert!(
            commitment.for_team(AiTeam::Blue),
            "the trailing team drops its detours and races the flag"
        );
        assert!(
            !commitment.for_team(AiTeam::Red),
            "the closing-time leader keeps playing the field"
        );
    }

    #[test]
    fn a_level_sudden_death_commits_both_teams() {
        use crate::gameplay::ctf::MatchPhase;

        let clock = MatchClock {
            frames_remaining: 1,
            phase: MatchPhase::SuddenDeath,
        };
        let commitment = objective_commitment(
            Some(&clock),
            CaptureScore {
                player: 2,
                opponents: 2,
            },
        );
        assert!(commitment.for_team(AiTeam::Blue));
        assert!(
            commitment.for_team(AiTeam::Red),
            "golden goal: both level sides race for the decider"
        );
    }

    #[test]
    fn a_missing_clock_never_forces_commitment() {
        assert_eq!(
            objective_commitment(
                None,
                CaptureScore {
                    player: 0,
                    opponents: 3,
                },
            ),
            ObjectiveCommitment::default(),
            "an unstarted match (no clock) plays the field as normal"
        );
    }

    #[test]
    fn no_team_protects_a_lead_outside_closing_time() {
        use crate::gameplay::ctf::{MatchPhase, CLOSING_TIME_FRAMES};

        let clock = MatchClock {
            frames_remaining: CLOSING_TIME_FRAMES + 1,
            phase: MatchPhase::Regulation,
        };
        assert_eq!(
            lead_protection(
                Some(&clock),
                CaptureScore {
                    player: 2,
                    opponents: 0,
                },
            ),
            LeadProtection::default(),
            "a round with time to spare must not pull the leader back to defend"
        );
    }

    #[test]
    fn closing_time_protects_only_the_team_that_is_ahead() {
        use crate::gameplay::ctf::{MatchPhase, CLOSING_TIME_FRAMES};

        let clock = MatchClock {
            frames_remaining: CLOSING_TIME_FRAMES,
            phase: MatchPhase::Regulation,
        };
        // Blue (player) leads Red (opponents) on captures.
        let protection = lead_protection(
            Some(&clock),
            CaptureScore {
                player: 2,
                opponents: 1,
            },
        );
        assert!(
            protection.for_team(AiTeam::Blue),
            "the leader digs in to guard its lead"
        );
        assert!(
            !protection.for_team(AiTeam::Red),
            "the trailing team commits to attack, it does not protect"
        );
    }

    #[test]
    fn a_level_sudden_death_protects_no_team() {
        use crate::gameplay::ctf::MatchPhase;

        let clock = MatchClock {
            frames_remaining: 1,
            phase: MatchPhase::SuddenDeath,
        };
        assert_eq!(
            lead_protection(
                Some(&clock),
                CaptureScore {
                    player: 2,
                    opponents: 2,
                },
            ),
            LeadProtection::default(),
            "golden goal: no one is ahead, so both sides race the decider"
        );
    }

    #[test]
    fn protection_is_the_exact_complement_of_commitment_in_closing_time() {
        use crate::gameplay::ctf::MatchPhase;

        let clock = MatchClock {
            frames_remaining: 1,
            phase: MatchPhase::SuddenDeath,
        };
        for (player, opponents) in [(0, 0), (2, 1), (1, 2)] {
            let captures = CaptureScore { player, opponents };
            let commit = objective_commitment(Some(&clock), captures);
            let protect = lead_protection(Some(&clock), captures);
            for team in [AiTeam::Blue, AiTeam::Red] {
                assert_ne!(
                    commit.for_team(team),
                    protect.for_team(team),
                    "in closing time a team either commits or protects, never both nor neither"
                );
            }
        }
    }

    #[test]
    fn closing_time_disciplines_both_the_leader_and_the_trailer() {
        use crate::gameplay::ctf::{MatchPhase, CLOSING_TIME_FRAMES};

        let clock = MatchClock {
            frames_remaining: CLOSING_TIME_FRAMES,
            phase: MatchPhase::Regulation,
        };
        // Blue (player) leads Red (opponents): the leader protects, the trailer
        // commits, yet in the closing stretch both leave cash bags on the track.
        let discipline = closing_time_pickup_discipline(
            Some(&clock),
            CaptureScore {
                player: 2,
                opponents: 1,
            },
        );
        assert!(
            discipline.for_team(AiTeam::Blue),
            "the leader stops farming cash while it protects its lead"
        );
        assert!(
            discipline.for_team(AiTeam::Red),
            "the trailing team stops farming cash while it commits to attack"
        );
    }

    #[test]
    fn no_team_disciplines_its_detours_outside_closing_time() {
        use crate::gameplay::ctf::{MatchPhase, CLOSING_TIME_FRAMES};

        let clock = MatchClock {
            frames_remaining: CLOSING_TIME_FRAMES + 1,
            phase: MatchPhase::Regulation,
        };
        assert_eq!(
            closing_time_pickup_discipline(
                Some(&clock),
                CaptureScore {
                    player: 2,
                    opponents: 1,
                },
            ),
            ClosingTimePickupDiscipline::default(),
            "with time to spare a cash bag is fair game for either side"
        );
        assert_eq!(
            closing_time_pickup_discipline(
                None,
                CaptureScore {
                    player: 2,
                    opponents: 1,
                },
            ),
            ClosingTimePickupDiscipline::default(),
            "an unstarted match (no clock) plays the field as normal"
        );
    }

    #[test]
    fn closing_time_discipline_is_the_union_of_commitment_and_protection() {
        use crate::gameplay::ctf::MatchPhase;

        let clock = MatchClock {
            frames_remaining: 1,
            phase: MatchPhase::SuddenDeath,
        };
        for (player, opponents) in [(0, 0), (2, 1), (1, 2)] {
            let captures = CaptureScore { player, opponents };
            let commit = objective_commitment(Some(&clock), captures);
            let protect = lead_protection(Some(&clock), captures);
            let discipline = closing_time_pickup_discipline(Some(&clock), captures);
            for team in [AiTeam::Blue, AiTeam::Red] {
                assert_eq!(
                    discipline.for_team(team),
                    commit.for_team(team) || protect.for_team(team),
                    "discipline must be the per-team union of commitment and protection"
                );
            }
        }
    }
}
