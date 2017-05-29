extern crate rand;
extern crate common;

use common::*;
use common::Role::*;
use common::Turn::*;
use common::Participant::*;
use common::Claim::*;
use common::CenterPair::*;
use common::CenterCard::*;
use common::ZeroToTwo::*;

use rand::{StdRng, SeedableRng, Rng};
use std::collections::HashMap;

//NOTE(Ryan1729): debug_assertions only appears to work correctly when the
//crate is not a dylib. Assuming you make this crate *not* a dylib on release,
//these configs should work
#[cfg(debug_assertions)]
#[no_mangle]
pub fn new_state(size: Size) -> State {
    //skip the title screen
    println!("debug on");

    let seed: &[_] = &[42];
    let mut rng: StdRng = SeedableRng::from_seed(seed);

    make_state(size, false, rng)
}
#[cfg(not(debug_assertions))]
#[no_mangle]
pub fn new_state(size: Size) -> State {
    //show the title screen
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|dur| dur.as_secs())
        .unwrap_or(42);

    println!("{}", timestamp);
    let seed: &[_] = &[timestamp as usize];
    let rng: StdRng = SeedableRng::from_seed(seed);

    make_state(size, true, rng)
}


fn make_state(size: Size, title_screen: bool, mut rng: StdRng) -> State {
    let role_spec = rng.gen::<RoleSpec>();

    let (player, cpu_roles, table_roles, player_knowledge, cpu_knowledge, _) =
        get_roles_and_knowledge(&role_spec, &mut rng);

    let initial_cpu_roles = cpu_roles.to_owned();

    State {
        rng: rng,
        title_screen: title_screen,
        player,
        initial_player: player,
        cpu_roles,
        initial_cpu_roles,
        table_roles,
        turn: Ready,
        player_knowledge,
        cpu_knowledge,
        votes: Vec::new(),
        claims: HashMap::new(),
        ui_context: UIContext::new(),
        role_spec,
        show_role_spec: false,
    }
}

fn get_roles_and_knowledge(role_spec: &RoleSpec,
                           rng: &mut StdRng)
                           -> (Role, Vec<Role>, [Role; 3], Knowledge, Vec<Knowledge>, bool) {
    let mut roles = role_spec.get_role_vector();

    rng.shuffle(&mut roles);

    let player = roles.pop().unwrap();

    let table_roles = [roles.pop().unwrap(), roles.pop().unwrap(), roles.pop().unwrap()];

    let mut cpu_roles = roles;

    if let Some(doppel_index) = linear_search(&cpu_roles, &DoppelVillager(Player)) {
        let mut other_roles: Vec<Role> = cpu_roles.iter()
            .map(|&r| r)
            .filter(|&r| r != DoppelVillager(Player))
            .collect();
        other_roles.push(player);

        let len = other_roles.len();
        let random_index = rng.gen_range(0, len);

        let participant = if random_index == len {
            Player
        } else if random_index >= doppel_index {
            // handle the fact that that doppel_index has been removed from other_roles
            Cpu(random_index + 1)
        } else {
            Cpu(random_index)
        };

        cpu_roles[doppel_index] = get_doppel_role(other_roles[random_index], participant);
    }

    let player_knowledge = Knowledge::new(player, Player);

    let mut cpu_knowledge = Vec::new();

    for i in 0..cpu_roles.len() {
        cpu_knowledge.push(Knowledge::new(cpu_roles[i], Cpu(i)));
    }

    (player,
     cpu_roles,
     table_roles,
     player_knowledge,
     cpu_knowledge,
     player == DoppelVillager(Player))
}

#[no_mangle]
//returns true if quit requested
pub fn update_and_render(platform: &Platform, state: &mut State, events: &mut Vec<Event>) -> bool {
    if state.title_screen {

        for event in events {
            cross_mode_event_handling(platform, state, event);
            match *event {
                Event::Close |
                Event::KeyPressed { key: KeyCode::Escape, ctrl: _, shift: _ } => return true,
                Event::KeyPressed { key: _, ctrl: _, shift: _ } => state.title_screen = false,
                _ => (),
            }
        }

        draw(platform, state);

        false
    } else {
        game_update_and_render(platform, state, events)
    }
}



pub fn game_update_and_render(platform: &Platform,
                              state: &mut State,
                              events: &mut Vec<Event>)
                              -> bool {
    let mut left_mouse_pressed = false;
    let mut left_mouse_released = false;

    for event in events {
        cross_mode_event_handling(platform, state, event);

        match *event {
            Event::KeyPressed { key: KeyCode::MouseLeft, ctrl: _, shift: _ } => {
                left_mouse_pressed = true;
            }
            Event::KeyReleased { key: KeyCode::MouseLeft, ctrl: _, shift: _ } => {
                left_mouse_released = true;
            }
            Event::Close |
            Event::KeyPressed { key: KeyCode::Escape, ctrl: _, shift: _ } => return true,
            _ => (),
        }
    }

    state.ui_context.frame_init();

    let next_spec = ButtonSpec {
        x: 0,
        y: 0,
        w: 10,
        h: 3,
        text: "Next".to_string(),
        id: 1,
    };

    if do_button(platform,
                 &mut state.ui_context,
                 &next_spec,
                 left_mouse_pressed,
                 left_mouse_released) {
        state.turn = state.turn.next();
    }

    let size = (platform.size)();

    let toggle_role_spec_spec = ButtonSpec {
        x: 0,
        y: size.height - 4,
        w: 24,
        h: 3,
        text: if state.show_role_spec {
            "Back to game".to_string()
        } else {
            "Show Available Roles".to_string()
        },
        id: 3,
    };

    if do_button(platform,
                 &mut state.ui_context,
                 &toggle_role_spec_spec,
                 left_mouse_pressed,
                 left_mouse_released) {
        state.show_role_spec = !state.show_role_spec;
    }



    if state.show_role_spec {
        display_role_spec(platform, 10, 10, &state.role_spec);
    } else {
        let t = state.turn;
        advance_turn_if_needed(state, platform, left_mouse_pressed, left_mouse_released);

        if t != state.turn {
            println!("{:?}", state.turn);
        }
    }

    draw(platform, state);

    false
}

fn advance_turn_if_needed(state: &mut State,
                          platform: &Platform,
                          left_mouse_pressed: bool,
                          left_mouse_released: bool) {
    match state.turn {
        Ready => {
            (platform.print_xy)(10, 8, "Ready to start a game?");

            //TODO pick roles and number of players

            let reroll_spec = ButtonSpec {
                x: 16,
                y: 0,
                w: 15,
                h: 3,
                text: "Randomize".to_string(),
                id: 6,
            };

            if do_button(platform,
                         &mut state.ui_context,
                         &reroll_spec,
                         left_mouse_pressed,
                         left_mouse_released) {
                state.role_spec = state.rng.gen::<RoleSpec>();
            }


            display_role_spec(platform, 10, 10, &state.role_spec);

            if ready_button(platform, state, left_mouse_pressed, left_mouse_released) {
                let (player,
                     cpu_roles,
                     table_roles,
                     player_knowledge,
                     cpu_knowledge,
                     player_is_doppel) = get_roles_and_knowledge(&state.role_spec, &mut state.rng);

                state.player = player;
                state.initial_player = player;
                state.initial_cpu_roles = cpu_roles.to_owned();
                state.cpu_roles = cpu_roles;
                state.table_roles = table_roles;
                state.player_knowledge = player_knowledge;
                state.cpu_knowledge = cpu_knowledge;

                state.turn = SeeRole(player_is_doppel);
            } else {
                state.turn = Ready;
            };
        }
        SeeRole(player_is_doppel) => {
            if player_is_doppel {
                (platform.print_xy)(10, 12, "You are a Doppelganger.");
                (platform.print_xy)(9, 13, "Choose a player to copy.");

                let choice =
                    pick_cpu_player(platform, state, left_mouse_pressed, left_mouse_released);

                match choice {
                    Some(p) => {
                        if let Some(role) = get_role(state, p) {
                            state.player = get_doppel_role(role, p);
                            state.turn = SeeRole(false);
                        }
                    }
                    None => {}
                }
            } else {
                (platform.print_xy)(10,
                                    12,
                                    &format!("You are {}.", full_role_string(state.player)));

                if ready_button(platform, state, left_mouse_pressed, left_mouse_released) {
                    state.turn = state.turn.next();
                };
            }
        }
        DoppelSeerTurn => {
            seer_turn(state,
                      platform,
                      left_mouse_pressed,
                      left_mouse_released,
                      DoppelSeerRevealOne,
                      DoppelSeerRevealTwo,
                      doppel_reveal_one_turn,
                      doppel_reveal_two_turn,
                      is_doppel_seer,
                      "DoppelSeer");
        }
        DoppelSeerRevealOne(participant) => {
            seer_reveal_one(state,
                            platform,
                            left_mouse_pressed,
                            left_mouse_released,
                            participant);
        }
        DoppelSeerRevealTwo(pair) => {
            seer_reveal_two(state,
                            platform,
                            left_mouse_pressed,
                            left_mouse_released,
                            pair);
        }
        DoppelRobberTurn => {
            robber_turn(state,
                        platform,
                        left_mouse_pressed,
                        left_mouse_released,
                        DoppelRobberReveal,
                        doppel_robber_action,
                        is_player_doppel_robber,
                        get_doppel_robber_index,
                        "DoppelRobber");
        }
        DoppelRobberReveal => {
            reveal_player(state, platform, left_mouse_pressed, left_mouse_released);
        }
        DoppelTroublemakerTurn => {
            troublemaker_turn(state,
                              platform,
                              left_mouse_pressed,
                              left_mouse_released,
                              is_player_doppel_troublemaker,
                              get_doppel_troublemaker_index,
                              DoppelTroublemakerSecondChoice,
                              doppel_troublemaker_action,
                              "DoppelTroublemaker");
        }
        DoppelTroublemakerSecondChoice(first_choice) => {
            troublemaker_second_choice(state,
                                       platform,
                                       left_mouse_pressed,
                                       left_mouse_released,
                                       doppel_troublemaker_action,
                                       DoppelTroublemakerTurn,
                                       first_choice);
        }
        DoppelDrunkTurn => {
            drunk_turn(state,
                       platform,
                       left_mouse_pressed,
                       left_mouse_released,
                       is_player_doppel_drunk,
                       get_doppel_drunk_index,
                       doppel_drunk_action,
                       "DoppelDrunk");
        }
        DoppelMinionTurn => {
            minion_turn(state,
                        platform,
                        left_mouse_pressed,
                        left_mouse_released,
                        is_player_doppel_minion,
                        get_doppel_minion_index,
                        "DoppelMinion");
        }
        Werewolves => {
            let werewolves = get_werewolves(state);

            let ready = if is_werewolf(state.player) {
                (platform.print_xy)(10, 10, "Werewolves, wake up and look for other werewolves.");

                list_werewolves(platform, &werewolves);

                ready_button(platform, state, left_mouse_pressed, left_mouse_released)
            } else {
                true
            };

            if ready {
                for &werewolf in werewolves.iter() {
                    match werewolf {
                        Player => {
                            state.player_knowledge.known_werewolves.extend(werewolves.iter());
                        }
                        Cpu(index) => {
                            let ref mut knowledge = state.cpu_knowledge[index];

                            knowledge.known_werewolves.extend(werewolves.iter());
                        }
                    }
                }

                state.turn = state.turn.next();
            }

        }
        MinionTurn => {
            minion_turn(state,
                        platform,
                        left_mouse_pressed,
                        left_mouse_released,
                        is_player_minion,
                        get_minion_index,
                        "Minion");
        }
        MasonTurn => {
            let masons = get_masons(state);

            let ready = if is_mason(state.player) {
                (platform.print_xy)(10, 10, "Masons, wake up and look for other Masons.");

                for i in 0..masons.len() {
                    let index = i as i32;

                    match masons[i] {
                        Player => (platform.print_xy)(10, 12 + index, "You are a mason. (duh!)"),
                        cpu => (platform.print_xy)(10, 12 + index, &format!("{} is a mason.", cpu)),
                    }
                }

                ready_button(platform, state, left_mouse_pressed, left_mouse_released)
            } else {
                true
            };

            if ready {
                for &mason in masons.iter() {
                    println!("mason {}", mason);
                    let mut other_masons: Vec<Participant> = masons.iter()
                        .filter(|&&p| p != mason)
                        .map(|&p| p)
                        .collect();

                    let len = other_masons.len();

                    let claim = if let Some(DoppelMason(p)) = get_role(state, mason) {
                        DoppelMasonAction(p, other_masons_to_zero_to_two(&other_masons))
                    } else {
                        MasonAction(other_masons_to_zero_to_two(&other_masons))
                    };

                    match mason {
                        Player => {
                            if len == 0 {
                                state.player_knowledge.known_non_active.insert(Mason);
                            } else {
                                state.player_knowledge.known_villagers.extend(masons.iter());
                            }

                            state.player_knowledge.true_claim = claim;
                        }
                        Cpu(index) => {
                            let ref mut knowledge = state.cpu_knowledge[index];
                            println!("pre {:?}", knowledge.known_villagers);
                            if len == 0 {
                                knowledge.known_non_active.insert(Mason);
                            } else {
                                knowledge.known_villagers.extend(masons.iter());
                            }
                            println!("post {:?}", knowledge.known_villagers);

                            knowledge.true_claim = claim;
                        }
                    }
                }

                state.turn = state.turn.next();
            }
        }
        SeerTurn => {
            seer_turn(state,
                      platform,
                      left_mouse_pressed,
                      left_mouse_released,
                      SeerRevealOne,
                      SeerRevealTwo,
                      reveal_one_turn,
                      reveal_two_turn,
                      is_seer,
                      "Seer");
        }
        SeerRevealOne(participant) => {
            seer_reveal_one(state,
                            platform,
                            left_mouse_pressed,
                            left_mouse_released,
                            participant);
        }
        SeerRevealTwo(pair) => {
            seer_reveal_two(state,
                            platform,
                            left_mouse_pressed,
                            left_mouse_released,
                            pair);
        }
        RobberTurn => {
            robber_turn(state,
                        platform,
                        left_mouse_pressed,
                        left_mouse_released,
                        RobberReveal,
                        robber_action,
                        is_player_robber,
                        get_robber_index,
                        "Robber");
        }
        RobberReveal => {
            reveal_player(state, platform, left_mouse_pressed, left_mouse_released);
        }
        TroublemakerTurn => {
            troublemaker_turn(state,
                              platform,
                              left_mouse_pressed,
                              left_mouse_released,
                              is_player_troublemaker,
                              get_troublemaker_index,
                              TroublemakerSecondChoice,
                              troublemaker_action,
                              "Troublemaker");
        }
        TroublemakerSecondChoice(first_choice) => {
            troublemaker_second_choice(state,
                                       platform,
                                       left_mouse_pressed,
                                       left_mouse_released,
                                       troublemaker_action,
                                       TroublemakerTurn,
                                       first_choice);
        }
        DrunkTurn => {
            drunk_turn(state,
                       platform,
                       left_mouse_pressed,
                       left_mouse_released,
                       is_player_drunk,
                       get_drunk_index,
                       drunk_action,
                       "Drunk");
        }
        InsomniacTurn => {
            insomniac_turn(state,
                           platform,
                           left_mouse_pressed,
                           left_mouse_released,
                           is_player_insomniac,
                           get_insomniac_index,
                           insomniac_action,
                           "Insomniac");
        }
        DoppelInsomniacTurn => {
            insomniac_turn(state,
                           platform,
                           left_mouse_pressed,
                           left_mouse_released,
                           is_player_doppel_insomniac,
                           get_doppel_insomniac_index,
                           doppel_insomniac_action,
                           "DoppelInsomniac");
        }
        BeginDiscussion => {
            state.claims.clear();

            let mut first_speakers = get_other_participants(state, Player);
            let len = first_speakers.len();
            if len > 0 {
                let first_speaker_count = state.rng.gen_range(0, len);

                state.rng.shuffle(&mut first_speakers);
                for _ in 0..first_speaker_count {
                    if let Some(participant) = first_speakers.pop() {
                        make_cpu_claim(state, participant);
                    }
                }
            }

            state.turn = state.turn.next();
        }
        Discuss => {
            //TODO player can make claims to affect cpu players claims
            if let Some(player_claim_or_silence) =
                get_player_claim_or_silence(platform,
                                            state,
                                            left_mouse_pressed,
                                            left_mouse_released) {
                match player_claim_or_silence {
                    ActualClaim(player_claim) => insert_claim(state, Player, player_claim),
                    Silence => {}
                }

                make_remaining_claims(state);
            }

            let claims = get_claim_vec(state);

            let lines = claims_to_lines(state, claims);

            for i in 0..lines.len() {
                let index = i as i32;

                (platform.print_xy)(10, 4 + (index), &lines[i]);
            }

            if ready_button(platform, state, left_mouse_pressed, left_mouse_released) {
                //if the player doesn't want to make a claim/see the reminaing claims,
                //that's their business, but the cpu players should get to see what
                //the thother cpu players say.
                make_remaining_claims(state);

                state.turn = state.turn.next();
            }

        }
        Vote => {

            let possible_player_vote =
                pick_cpu_player(platform, state, left_mouse_pressed, left_mouse_released);

            if let Some(player_vote) = possible_player_vote {
                state.votes.clear();

                state.votes.push((Player, player_vote));

                for i in 0..state.cpu_knowledge.len() {
                    let voter = Cpu(i);

                    let insomniac_peek = {
                        let knowledge = &mut state.cpu_knowledge[i];

                        apply_swaps(knowledge);

                        knowledge.insomniac_peek
                    };

                    if insomniac_peek {
                        if let Some(role) = get_role(state, voter) {
                            let knowledge = &mut state.cpu_knowledge[i];

                            knowledge.role = role;
                            if is_werewolf(knowledge.role) {
                                knowledge.known_villagers.remove(&voter);
                                knowledge.known_werewolves.insert(voter);
                            } else if knowledge.role == Minion {
                                knowledge.known_villagers.remove(&voter);
                                knowledge.known_minion = Some(voter);
                            } else if knowledge.role == Tanner {
                                knowledge.known_villagers.remove(&voter);
                                knowledge.known_tanner = Some(voter);
                            };

                        }
                    }

                    let vote = get_vote(voter,
                                        get_participants(state),
                                        &state.cpu_knowledge[i],
                                        &mut state.rng);

                    state.votes.push((voter, vote));
                }

                state.turn = state.turn.next();
            }
        }
        Resolution => {
            for i in 0..state.votes.len() {
                let (voter, vote) = state.votes[i];
                (platform.print_xy)(10,
                                    (i as i32 + 1),
                                    &format!("{} voted for {}!", voter, vote));
            }

            let just_votes = &state.votes
                                  .iter()
                                  .map(|&(_, v)| v)
                                  .collect();
            let mut targets = count_votes(&just_votes);

            if let Some(hunter_participant) = get_participant_with_role(state, Hunter) {
                if targets.contains(&hunter_participant) {
                    if let Some(hunter_target) =
                        state.votes
                            .iter()
                            .find(|&&(voter, _)| voter == hunter_participant)
                            .map(|&(_, v)| v) {
                        if !targets.contains(&hunter_target) {
                            targets.push(hunter_target)
                        }
                    }
                }
            };

            if let Some(doppel_hunter_participant) =
                get_participant_by_role(state, |r| match r {
                    &DoppelHunter(_) => true,
                    _ => false,
                }) {
                if targets.contains(&doppel_hunter_participant) {
                    if let Some(doppel_hunter_target) =
                        state.votes
                            .iter()
                            .find(|&&(voter, _)| voter == doppel_hunter_participant)
                            .map(|&(_, v)| v) {
                        if !targets.contains(&doppel_hunter_target) {
                            targets.push(doppel_hunter_target)
                        }
                    }
                }
            };

            targets.sort();

            if targets.len() == 0 {
                (platform.print_xy)(10, 10, "Nobody died.");

                let werewolves = get_werewolves(state);

                let len = werewolves.len();
                if len == 0 {
                    (platform.print_xy)(10, 12, "And nobody was a werewolf!");
                    (platform.print_xy)(10, 13, "Village team wins!");
                } else {
                    if len > 1 {
                        (platform.print_xy)(10, 12, &format!("But there were {} werewolves!", len));
                    } else {
                        (platform.print_xy)(10, 12, "But there was a werewolf!");
                    }
                    (platform.print_xy)(10, 13, "Werewolf team wins!");
                }
            } else {
                (platform.print_xy)(10, 10, &format!("{} died!", str_list(&targets)));

                let target_roles = targets.iter().filter_map(|&p| get_role(state, p));
                let hit_werevoles_count = target_roles.filter(|&r| is_werewolf(r)).count();

                let possible_dead_tanner =
                    get_participant_with_role(state, Tanner).and_then(|p| if
                        targets.contains(&p) {
                                                                          Some(p)
                                                                      } else {
                                                                          None
                                                                      });
                let possible_dead_doppel_tanner =
                    get_participant_by_role(state, |r| match r {
                        &DoppelTanner(_) => true,
                        _ => false,
                    })
                            .and_then(|p| if targets.contains(&p) { Some(p) } else { None });

                if hit_werevoles_count >= 1 {
                    if hit_werevoles_count == 1 {
                        (platform.print_xy)(10, 12, "A werewolf died!");
                    } else {
                        (platform.print_xy)(10,
                                            12,
                                            &format!("{} werewolves died!", hit_werevoles_count));
                    }
                    (platform.print_xy)(10, 13, "Village team wins!");

                    if let Some(dead_tanner) = possible_dead_tanner {
                        display_tanner_win(platform, dead_tanner, true);
                    }
                    if let Some(dead_doppel_tanner) = possible_dead_doppel_tanner {
                        display_doppel_tanner_win(platform, dead_doppel_tanner, true);
                    }

                } else {
                    let werewolves = get_werewolves(state);

                    if werewolves.len() > 0 {
                        (platform.print_xy)(10,
                                            12,
                                            "No werewolves died but a player was a werewolf!");

                        match (possible_dead_tanner, possible_dead_doppel_tanner) {
                            (None, None) => {
                                (platform.print_xy)(10, 13, "Werewolf team wins!");
                            }
                            (Some(dead_tanner), None) => {
                                display_tanner_win(platform, dead_tanner, false);
                            }
                            (None, Some(dead_doppel_tanner)) => {
                                display_doppel_tanner_win(platform, dead_doppel_tanner, false);
                            }
                            (Some(dead_tanner), Some(dead_doppel_tanner)) => {
                                display_tanner_win(platform, dead_tanner, false);
                                display_doppel_tanner_win(platform, dead_doppel_tanner, true);

                            }
                        };

                    } else {
                        (platform.print_xy)(10,
                                            12,
                                            "No werewolves died but nobody was a werewolf!");

                        if let Some(_) = get_participant_with_role(state, Minion) {
                            (platform.print_xy)(10, 13, "But there was a minion! The minion wins!");

                            if let Some(dead_tanner) = possible_dead_tanner {
                                display_tanner_win(platform, dead_tanner, true);
                            }
                            if let Some(dead_doppel_tanner) = possible_dead_doppel_tanner {
                                display_doppel_tanner_win(platform, dead_doppel_tanner, true);
                            }
                        } else {
                            match (possible_dead_tanner, possible_dead_doppel_tanner) {
                                (None, None) => {
                                    (platform.print_xy)(10, 13, "Nobody wins!");
                                }
                                (Some(dead_tanner), None) => {
                                    display_tanner_win(platform, dead_tanner, false);
                                }
                                (None, Some(dead_doppel_tanner)) => {
                                    display_doppel_tanner_win(platform, dead_doppel_tanner, false);
                                }
                                (Some(dead_tanner), Some(dead_doppel_tanner)) => {
                                    display_tanner_win(platform, dead_tanner, false);
                                    display_doppel_tanner_win(platform, dead_doppel_tanner, true);

                                }
                            };
                        }
                    }
                }
            }

            (platform.print_xy)(10,
                                20,
                                &format!("You are {}", full_role_string(state.player)));

            for i in 0..state.cpu_roles.len() {
                (platform.print_xy)(10,
                                    21 + i as i32,
                                    &format!("{} is {}",
                                             Cpu(i),
                                             full_role_string(state.cpu_roles[i])));
            }

            if ready_button(platform, state, left_mouse_pressed, left_mouse_released) {
                state.turn = state.turn.next();
            }
        }

    }

}

fn other_masons_to_zero_to_two(other_masons: &Vec<Participant>) -> ZeroToTwo<Participant> {
    let len = other_masons.len();

    if len >= 2 {
        Two(other_masons[0], other_masons[1])
    } else if len == 1 {
        One(other_masons[0])
    } else {
        Zero
    }
}

fn display_role_spec(platform: &Platform, x: i32, y: i32, role_spec: &RoleSpec) {
    (platform.print_xy)(x,
                        y,
                        &format!("Cpu Players: {}", role_spec.cpu_player_count));

    let role_vec = role_spec.get_role_vector();

    //Here's the Run Length Encoder (RLE), in case you're grepping for it.
    let pairs = role_vec.iter().fold(Vec::new(), |mut acc, &role| {
        if let Some(RoleCount(last_role, count)) = acc.pop() {
            if last_role == role {
                acc.push(RoleCount(last_role, count + 1));
            } else {
                acc.push(RoleCount(last_role, count));
                acc.push(RoleCount(role, 1));
            }
        } else {
            acc.push(RoleCount(role, 1));
        }

        acc
    });

    let mut current_y = y + 1;
    let mut line = String::new();
    let mut counter = 0;
    let len = pairs.len();
    for i in 0..len - 1 {
        if i == len - 2 {
            if counter >= 2 {
                //Special case:
                //don't let the last line have MAX_ROLE_COUNTS_PER_LINE + 1 roles counts
                line.push_str(&format!("{}", pairs[i]));
                (platform.print_xy)(x, current_y, &line);
                current_y += 1;
                line.clear();

                line.push_str(&format!("and {}", pairs[i + 1]));
                (platform.print_xy)(x, current_y, &line);

                break;
            } else {
                line.push_str(&format!("{} and {}", pairs[i], pairs[i + 1]));
                counter = 3;
            }

        } else {
            line.push_str(&format!("{}, ", pairs[i]));
            counter += 1;
        }

        if counter >= MAX_ROLE_COUNTS_PER_LINE {
            (platform.print_xy)(x, current_y, &line);
            current_y += 1;
            counter = 0;

            line.clear();
        }
    }
}

const MAX_ROLE_COUNTS_PER_LINE: usize = 3;

#[derive(Debug)]
struct RoleCount(Role, u32);

impl std::fmt::Display for RoleCount {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {

        write!(f,
               "{} {:o} {}",
               self.1,
               self.0,
               if self.1 == 1 { "card" } else { "cards" })
    }
}

fn is_player_insomniac(state: &State) -> bool {
    state.initial_player == Insomniac
}

fn is_player_doppel_insomniac(state: &State) -> bool {
    match state.player {
        DoppelInsomniac(_) => true,
        _ => false,
    }
}

fn get_insomniac_index(state: &State) -> Option<usize> {
    linear_search(&state.initial_cpu_roles, &Insomniac)
}
fn get_doppel_insomniac_index(state: &State) -> Option<usize> {
    linear_search_by(&state.cpu_roles, |r| match r {
        &DoppelInsomniac(_) => true,
        _ => false,
    })
}

fn insomniac_action(_: &State, _: Participant, r: Role) -> Claim {
    InsomniacAction(r)
}
fn doppel_insomniac_action(state: &State, participant: Participant, r: Role) -> Claim {
    let participant = insomniac_doppel_target_or_player(get_role(state, participant)
                                                            .unwrap_or(Villager));
    DoppelInsomniacAction(participant, r)
}

fn insomniac_doppel_target_or_player(role: Role) -> Participant {
    match role {
        DoppelInsomniac(p) => p,
        _ => Player,
    }
}

fn insomniac_turn(state: &mut State,
                  platform: &Platform,
                  left_mouse_pressed: bool,
                  left_mouse_released: bool,
                  player_pred: fn(&State) -> bool,
                  get_cpu_index: fn(&State) -> Option<usize>,
                  action: fn(&State, Participant, Role) -> Claim,
                  name: &str) {

    if player_pred(state) {
        (platform.print_xy)(15, 3, "Insomniac, wake up and look at your card.");

        (platform.print_xy)(15, 5, &format!("You are {}", state.player));
        state.player_knowledge.true_claim = action(state, Player, state.player);

        if ready_button(platform, state, left_mouse_pressed, left_mouse_released) {
            state.turn = state.turn.next();
        }
    } else {
        if let Some(i) = get_cpu_index(state) {

            if let Some(&role) = state.cpu_roles.get(i) {
                let true_claim = action(state, Player, role);
                if let Some(knowledge) = state.cpu_knowledge.get_mut(i) {

                    knowledge.true_claim = true_claim;
                    knowledge.insomniac_peek = true;
                    knowledge.role = role;
                }
            }
        }

        state.turn = state.turn.next();
    }

}

fn is_player_drunk(state: &State) -> bool {
    state.initial_player == Drunk
}

fn is_player_doppel_drunk(state: &State) -> bool {
    match state.player {
        DoppelDrunk(_) => true,
        _ => false,
    }
}

fn get_drunk_index(state: &State) -> Option<usize> {
    linear_search(&state.initial_cpu_roles, &Drunk)
}
fn get_doppel_drunk_index(state: &State) -> Option<usize> {
    linear_search_by(&state.cpu_roles, |r| match r {
        &DoppelDrunk(_) => true,
        _ => false,
    })
}

fn drunk_action(_: &State, _: Participant, c: CenterCard) -> Claim {
    DrunkAction(c)
}
fn doppel_drunk_action(state: &State, participant: Participant, c: CenterCard) -> Claim {
    let participant = drunk_doppel_target_or_player(get_role(state, participant)
                                                        .unwrap_or(Villager));
    DoppelDrunkAction(participant, c)
}

fn drunk_doppel_target_or_player(role: Role) -> Participant {
    match role {
        DoppelDrunk(p) => p,
        _ => Player,
    }
}

fn drunk_turn(state: &mut State,
              platform: &Platform,
              left_mouse_pressed: bool,
              left_mouse_released: bool,
              player_pred: fn(&State) -> bool,
              get_cpu_index: fn(&State) -> Option<usize>,
              action: fn(&State, Participant, CenterCard) -> Claim,
              name: &str) {
    if player_pred(state) {
        (platform.print_xy)(15,
                            3,
                            "Drunk, wake up
and exchange your card with a card from the center.");

        let choice = pick_displayable(platform,
                                      state,
                                      left_mouse_pressed,
                                      left_mouse_released,
                                      &CenterCard::all_values());
        match choice {
            Some(chosen) => {
                swap_role_with_center(state, Player, chosen);

                let true_claim = action(state, Player, chosen);
                if let Some(knowledge) = get_knowledge_mut(state, Player) {
                    knowledge.true_claim = true_claim;
                    knowledge.drunk_swap = Some((Player, chosen));
                }

                state.turn = state.turn.next();
            }
            None => {}
        }
    } else {
        if let Some(drunk_index) = get_cpu_index(state) {
            let drunk = Cpu(drunk_index);

            let target = state.rng.gen::<CenterCard>();

            swap_role_with_center(state, drunk, target);

            let true_claim = action(state, drunk, target);
            if let Some(knowledge) = get_knowledge_mut(state, drunk) {
                knowledge.true_claim = true_claim;
                knowledge.drunk_swap = Some((drunk, target));
            }
        }

        state.turn = state.turn.next();
    };
}

fn is_player_troublemaker(state: &State) -> bool {
    state.initial_player == Troublemaker
}

fn is_player_doppel_troublemaker(state: &State) -> bool {
    match state.player {
        DoppelTroublemaker(_) => true,
        _ => false,
    }
}

fn get_troublemaker_index(state: &State) -> Option<usize> {
    linear_search(&state.initial_cpu_roles, &Troublemaker)
}
fn get_doppel_troublemaker_index(state: &State) -> Option<usize> {
    linear_search_by(&state.cpu_roles, |r| match r {
        &DoppelTroublemaker(_) => true,
        _ => false,
    })
}

fn troublemaker_action(_: &State, _: Participant, p1: Participant, p2: Participant) -> Claim {
    TroublemakerAction(p1, p2)
}
fn doppel_troublemaker_action(state: &State,
                              participant: Participant,
                              p1: Participant,
                              p2: Participant)
                              -> Claim {
    let participant = troublemaker_doppel_target_or_player(get_role(state, participant)
                                                               .unwrap_or(Villager));
    DoppelTroublemakerAction(participant, p1, p2)
}

fn troublemaker_doppel_target_or_player(role: Role) -> Participant {
    match role {
        DoppelTroublemaker(p) => p,
        _ => Player,
    }
}

fn troublemaker_second_choice(state: &mut State,
                              platform: &Platform,
                              left_mouse_pressed: bool,
                              left_mouse_released: bool,
                              action: fn(&State, Participant, Participant, Participant) -> Claim,
                              back_turn: Turn,
                              first_choice: Participant) {
    (platform.print_xy)(15, 5, "Choose the second other player:");

    let remaining_options = get_cpu_participants(state)
        .iter()
        .filter(|&&p| p != first_choice)
        .map(|&p| p)
        .collect();
    if let Some(second_choice) =
        pick_displayable(platform,
                         state,
                         left_mouse_pressed,
                         left_mouse_released,
                         &remaining_options) {
        swap_roles(state, first_choice, second_choice);
        state.player_knowledge.true_claim = action(state, Player, first_choice, second_choice);

        state.turn = state.turn.next();
    };
    if do_button(platform,
                 &mut state.ui_context,
                 &ButtonSpec {
                      x: 0,
                      y: 8,
                      w: 11,
                      h: 3,
                      text: "Back".to_string(),
                      id: 11,
                  },
                 left_mouse_pressed,
                 left_mouse_released) {
        state.turn = back_turn;
    }
}
fn troublemaker_turn(state: &mut State,
                     platform: &Platform,
                     left_mouse_pressed: bool,
                     left_mouse_released: bool,
                     player_pred: fn(&State) -> bool,
                     get_cpu_index: fn(&State) -> Option<usize>,
                     second_choice: fn(Participant) -> Turn,
                     action: fn(&State, Participant, Participant, Participant) -> Claim,
                     name: &str) {
    if player_pred(state) {
        (platform.print_xy)(15,
                            3,
                            "Troublemaker, wake up.
You may exchange cards between two other players.");

        (platform.print_xy)(15, 5, "Choose the first other player:");


        let choice =
            pick_cpu_player_or_skip(platform, state, left_mouse_pressed, left_mouse_released);
        match choice {
            Skip => {
                state.turn = state.turn.next();
            }
            Chosen(chosen) => {
                state.turn = second_choice(chosen);
            }
            NoChoice => {}
        }
    } else {
        if let Some(troublemaker_index) = get_cpu_index(state) {
            let troublemaker = Cpu(troublemaker_index);

            let mut other_participants = get_other_participants(state, troublemaker);
            state.rng.shuffle(&mut other_participants);

            if let (Some(first_choice), Some(second_choice)) =
                (other_participants.pop(), other_participants.pop()) {
                swap_roles(state, first_choice, second_choice);

                let true_claim = action(state, troublemaker, first_choice, second_choice);
                if let Some(knowledge) = get_knowledge_mut(state, troublemaker) {
                    knowledge.true_claim = true_claim;
                    knowledge.troublemaker_swap = Some((first_choice, second_choice));
                }
            }
        }

        state.turn = state.turn.next();
    };
}

fn is_player_minion(state: &State) -> bool {
    state.initial_player == Minion
}

fn is_player_doppel_minion(state: &State) -> bool {
    match state.player {
        DoppelMinion(_) => true,
        _ => false,
    }
}

fn get_minion_index(state: &State) -> Option<usize> {
    linear_search(&state.initial_cpu_roles, &Minion)
}
fn get_doppel_minion_index(state: &State) -> Option<usize> {
    linear_search_by(&state.cpu_roles, |r| match r {
        &DoppelMinion(_) => true,
        _ => false,
    })
}
fn minion_turn(state: &mut State,
               platform: &Platform,
               left_mouse_pressed: bool,
               left_mouse_released: bool,
               player_pred: fn(&State) -> bool,
               get_cpu_index: fn(&State) -> Option<usize>,
               name: &str) {
    let werewolves = get_werewolves(state);

    if player_pred(state) {
        (platform.print_xy)(10,
                            10,
                            &format!("{}, wake up. Werewolves, stick out
your thumb so the Minion can see who you are.",
                                     name));

        list_werewolves(platform, &werewolves);

        if ready_button(platform, state, left_mouse_pressed, left_mouse_released) {
            state.turn = state.turn.next();
        }
    } else {
        if let Some(minion_index) = get_cpu_index(state) {
            let minion = Cpu(minion_index);

            if let Some(knowledge) = get_knowledge_mut(state, minion) {
                knowledge.known_werewolves.extend(werewolves.iter());
                knowledge.known_minion = Some(minion);
                knowledge.true_claim = Simple(Minion);
            }
        }

        state.turn = state.turn.next();
    }
}

fn robber_action(_: &State, _: Participant, p: Participant, r: Role) -> Claim {
    RobberAction(p, r)
}
fn doppel_robber_action(state: &State, participant: Participant, p: Participant, r: Role) -> Claim {
    let participant = robber_doppel_target_or_player(get_role(state, participant)
                                                         .unwrap_or(Villager));
    DoppelRobberAction(participant, p, r)
}

fn robber_doppel_target_or_player(role: Role) -> Participant {
    match role {
        DoppelRobber(p) => p,
        _ => Player,
    }
}


fn reveal_player(state: &mut State,
                 platform: &Platform,
                 left_mouse_pressed: bool,
                 left_mouse_released: bool) {
    (platform.print_xy)(10, 10, &format!("You are now {}.", state.player));

    if ready_button(platform, state, left_mouse_pressed, left_mouse_released) {
        state.turn = state.turn.next();
    }
}
fn robber_turn(state: &mut State,
               platform: &Platform,
               left_mouse_pressed: bool,
               left_mouse_released: bool,
               reveal_turn: Turn,
               action: fn(&State, Participant, Participant, Role) -> Claim,
               player_pred: fn(&State) -> bool,
               get_cpu_index: fn(&State) -> Option<usize>,
               name: &str) {
    if player_pred(state) {
        (platform.print_xy)(15,
                            3,
                            &format!("{}, wake up.
You may exchange your card with another player’s card,
and then view your new card.",
                                     name));


        let choice =
            pick_cpu_player_or_skip(platform, state, left_mouse_pressed, left_mouse_released);
        match choice {
            Skip => {
                state.turn = state.turn.next();
            }
            Chosen(chosen) => {
                swap_roles(state, Player, chosen);

                state.turn = reveal_turn;
            }
            NoChoice => {}
        }
    } else {
        if let Some(robber_index) = get_cpu_index(state) {
            let robber = Cpu(robber_index);

            let other_participants = get_other_participants(state, robber);
            if let Some(&chosen) = state.rng.choose(&other_participants) {
                swap_roles(state, robber, chosen);

                if let Some(new_role) = get_role(state, robber) {
                    let true_claim = action(state, robber, chosen, new_role);
                    if let Some(knowledge) = get_knowledge_mut(state, robber) {
                        knowledge.role = new_role;
                        knowledge.true_claim = true_claim;
                        knowledge.robber_swap = Some((robber, chosen, new_role));
                    }
                }
            }
        }

        state.turn = state.turn.next();
    };
}
fn is_player_robber(state: &State) -> bool {
    state.initial_player == Robber
}
fn is_player_doppel_robber(state: &State) -> bool {
    match state.player {
        DoppelRobber(_) => true,
        _ => false,
    }
}
fn get_robber_index(state: &State) -> Option<usize> {
    linear_search(&state.initial_cpu_roles, &Robber)
}
fn get_doppel_robber_index(state: &State) -> Option<usize> {
    linear_search_by(&state.cpu_roles, |r| match r {
        &DoppelRobber(_) => true,
        _ => false,
    })
}

fn reveal_one_turn(state: &State, participant: Participant, p: Participant, r: Role) -> Claim {
    SeerRevealOneAction(p, r)
}
fn reveal_two_turn(state: &State,
                   participant: Participant,
                   pair: CenterPair,
                   r1: Role,
                   r2: Role)
                   -> Claim {
    SeerRevealTwoAction(pair, r1, r2)
}

fn doppel_reveal_one_turn(state: &State,
                          participant: Participant,
                          p: Participant,
                          r: Role)
                          -> Claim {
    let participant = seer_doppel_target_or_player(get_role(state, participant)
                                                       .unwrap_or(Villager));

    DoppelSeerRevealOneAction(participant, p, r)
}
fn doppel_reveal_two_turn(state: &State,
                          participant: Participant,
                          pair: CenterPair,
                          r1: Role,
                          r2: Role)
                          -> Claim {
    let participant = seer_doppel_target_or_player(get_role(state, participant)
                                                       .unwrap_or(Villager));
    DoppelSeerRevealTwoAction(participant, pair, r1, r2)
}


fn seer_doppel_target_or_player(role: Role) -> Participant {
    match role {
        DoppelSeer(p) => p,
        _ => Player,
    }
}

fn is_seer(role: &Role) -> bool {
    role == &Seer
}

fn is_doppel_seer(role: &Role) -> bool {
    match role {
        &DoppelSeer(_) => true,
        _ => false,
    }
}

fn seer_reveal_two(state: &mut State,
                   platform: &Platform,
                   left_mouse_pressed: bool,
                   left_mouse_released: bool,
                   pair: CenterPair) {
    let (role1, role2) = get_role_pair(state, pair);

    let (ordinal1, ordinal2) = match pair {
        FirstSecond => ("First", "Second"),
        FirstThird => ("First", "Third"),
        SecondThird => ("Second", "Third"),
    };

    (platform.print_xy)(10, 10, &format!("The {} card is {}.", ordinal1, role1));
    (platform.print_xy)(10, 11, &format!("And the {} card is {}.", ordinal2, role2));


    if ready_button(platform, state, left_mouse_pressed, left_mouse_released) {
        state.turn = state.turn.next();
    }
}

fn seer_reveal_one(state: &mut State,
                   platform: &Platform,
                   left_mouse_pressed: bool,
                   left_mouse_released: bool,
                   participant: Participant) {
    if let Some(role) = get_role(state, participant) {
        (platform.print_xy)(10, 10, &format!("{} is {}.", participant, role));
    } else {
        (platform.print_xy)(10,
                            10,
                            &format!("{} apparently isn't playing?!", participant));
    }

    if ready_button(platform, state, left_mouse_pressed, left_mouse_released) {
        state.turn = state.turn.next();
    }
}

fn seer_turn(state: &mut State,
             platform: &Platform,
             left_mouse_pressed: bool,
             left_mouse_released: bool,
             reveal_one: fn(Participant) -> Turn,
             reveal_two: fn(CenterPair) -> Turn,
             reveal_one_action: fn(&State, Participant, Participant, Role) -> Claim,
             reveal_two_action: fn(&State, Participant, CenterPair, Role, Role) -> Claim,
             role_pred: fn(&Role) -> bool,
             name: &str) {


    if role_pred(&state.player) {
        (platform.print_xy)(15,
                            3,
                            &format!("{}, wake up.
You may look at another
player’s card or two of the center cards.",
                                     name));


        let choice = pick_seer_choice(platform, state, left_mouse_pressed, left_mouse_released);
        match choice {
            SeerCpuOrSkip(cpu_or_skip) => {
                match cpu_or_skip {

                    Skip => {
                        state.turn = state.turn.next();
                    }
                    Chosen(chosen) => {
                        state.turn = reveal_one(chosen);
                    }
                    NoChoice => {}
                }
            }
            ChosenPair(chosen) => {
                state.turn = reveal_two(chosen);
            }
        }
    } else {
        if let Some(seer_index) = linear_search_by(&state.cpu_roles, role_pred) {
            let seer = Cpu(seer_index);


            let look_at_two = state.rng.gen::<bool>();

            //TODO choose player or center based on active roles?
            if look_at_two {

                let pair = state.rng.gen::<CenterPair>();

                let (role1, role2) = get_role_pair(state, pair);
                let true_claim = reveal_two_action(state, seer, pair, role1, role2);

                if let Some(knowledge) = get_knowledge_mut(state, seer) {
                    knowledge.known_non_active.insert(role1);
                    knowledge.known_non_active.insert(role2);

                    knowledge.true_claim = true_claim;
                }
            } else {
                let other_participants = get_other_participants(state, seer);
                if let Some(&chosen) = state.rng.choose(&other_participants) {
                    if let Some(seen_role) = get_role(state, chosen) {
                        let true_claim = reveal_one_action(state, seer, chosen, seen_role);

                        if let Some(knowledge) = get_knowledge_mut(state, seer) {
                            if is_werewolf(seen_role) {
                                knowledge.known_villagers.remove(&chosen);
                                knowledge.known_werewolves.insert(chosen);
                            } else if seen_role == Minion {
                                knowledge.known_villagers.remove(&chosen);
                                knowledge.known_werewolves.remove(&chosen);
                                knowledge.known_minion = Some(chosen);
                            } else if seen_role == Tanner {
                                knowledge.known_villagers.remove(&chosen);
                                knowledge.known_werewolves.remove(&chosen);
                                knowledge.known_tanner = Some(chosen);
                            } else {
                                knowledge.known_villagers.insert(chosen);
                            };

                            knowledge.true_claim = true_claim;
                        }
                    }
                }

            }
        }

        state.turn = state.turn.next();
    };
}

fn display_tanner_win(platform: &Platform, dead_tanner: Participant, addtional: bool) {
    let pronoun = if dead_tanner == Player { "You" } else { "they" };
    (platform.print_xy)(10,
                        14,
                        &format!("{} died and {} were {}.", dead_tanner, pronoun, Tanner));
    if addtional {

        (platform.print_xy)(10, 15, "Tanner wins too!");
    } else {
        (platform.print_xy)(10, 15, "Tanner wins!");
    }
}

fn display_doppel_tanner_win(platform: &Platform,
                             dead_doppel_tanner: Participant,
                             addtional: bool) {
    let pronoun = if dead_doppel_tanner == Player {
        "You"
    } else {
        "they"
    };
    (platform.print_xy)(10,
                        16,
                        &format!("{} died and {} were {}.",
                                 dead_doppel_tanner,
                                 pronoun,
                                 DoppelTanner(Player)));
    if addtional {

        (platform.print_xy)(10, 17, "DoppelTanner wins as well!");
    } else {

        (platform.print_xy)(10, 17, "DoppelTanner wins!");
    }
}

fn list_werewolves(platform: &Platform, werewolves: &Vec<Participant>) {
    let len = werewolves.len();

    if len > 0 {
        for i in 0..len {
            let index = i as i32;

            match werewolves[i] {
                Player => (platform.print_xy)(10, 12 + index, "You are a werewolf. (duh!)"),
                cpu => (platform.print_xy)(10, 12 + index, &format!("{} is a werewolf.", cpu)),
            }
        }
    } else {
        (platform.print_xy)(10,
                            12,
                            "There are no werewolves. They must be in the center.")
    }
}

fn get_participant_with_role(state: &State, role: Role) -> Option<Participant> {
    if state.player == role {
        Some(Player)
    } else {
        linear_search(&state.cpu_roles, &role).map(|i| Cpu(i))
    }
}

fn get_participant_by_role<'a, F>(state: &'a State, mut f: F) -> Option<Participant>
    where F: FnMut(&'a Role) -> bool
{
    if f(&state.player) {
        Some(Player)
    } else {
        linear_search_by(&state.cpu_roles, f).map(|i| Cpu(i))
    }
}

fn linear_search<T: PartialEq>(vector: &Vec<T>, thing: &T) -> Option<usize> {
    for i in 0..vector.len() {
        if thing == &vector[i] {
            return Some(i);
        }
    }

    None
}

fn linear_search_by<'a, F, T>(vector: &'a Vec<T>, mut f: F) -> Option<usize>
    where F: FnMut(&'a T) -> bool
{
    for i in 0..vector.len() {
        if f(&vector[i]) {
            return Some(i);
        }
    }

    None
}


fn apply_swaps(knowledge: &mut Knowledge) {
    //TODO maybe make this return a new type, "FinalKnowledge"?

    //TODO Do wwe need doppel swapping? There will only ever be one and it will
    //happen before a seer happens.
    if let Some((robber, target, previous_role)) = knowledge.robber_swap {

        knowledge.role = previous_role;
        if is_werewolf(previous_role) {
            knowledge.known_villagers.remove(&robber);
            knowledge.known_werewolves.insert(robber);
        } else if knowledge.role == Minion {
            knowledge.known_villagers.remove(&robber);
            knowledge.known_minion = Some(robber);
        } else if knowledge.role == Tanner {
            knowledge.known_villagers.remove(&robber);
            knowledge.known_tanner = Some(robber);
        };

        knowledge.known_werewolves.remove(&target);
        knowledge.known_villagers.insert(target);
    }

    if let Some((target1, target2)) = knowledge.troublemaker_swap {
        swap_team_if_known(knowledge, target1);
        swap_team_if_known(knowledge, target2);
    }

    //TODO Do we need Drunk swapping (or any swapping?!) if the cpus never trusst anyone?
}

fn swap_team_if_known(knowledge: &mut Knowledge, participant: Participant) {
    if knowledge.known_werewolves.contains(&participant) {
        knowledge.known_villagers.insert(participant);
        knowledge.known_werewolves.remove(&participant);
    } else if knowledge.known_villagers.contains(&participant) {
        knowledge.known_werewolves.insert(participant);
        knowledge.known_villagers.remove(&participant);
    }
}



fn get_role_pair(state: &State, pair: CenterPair) -> (Role, Role) {
    let rs = state.table_roles;
    match pair {
        FirstSecond => (rs[0], rs[1]),
        FirstThird => (rs[0], rs[2]),
        SecondThird => (rs[1], rs[2]),
    }
}

const MAX_CLAIM_HEIGHT: i32 = 4;

fn get_initial_role(state: &State, participant: Participant) -> Option<&Role> {
    match participant {
        Player => Some(&state.initial_player),
        Cpu(index) => state.initial_cpu_roles.get(index),
    }
}

fn get_knowledge(state: &State, participant: Participant) -> Option<&Knowledge> {
    match participant {
        Player => Some(&state.player_knowledge),
        Cpu(index) => state.cpu_knowledge.get(index),
    }
}
fn get_knowledge_mut(state: &mut State, participant: Participant) -> Option<&mut Knowledge> {
    match participant {
        Player => Some(&mut state.player_knowledge),
        Cpu(index) => state.cpu_knowledge.get_mut(index),
    }
}
fn get_knowledge_copy(state: &State, participant: Participant) -> Option<Knowledge> {
    get_knowledge(state, participant).map(|k| k.clone())
}


fn make_cpu_claim(state: &mut State, participant: Participant) {
    if participant == Player {
        return;
    }

    //the copy is needed so we can get random numbers below
    let possible_knowledge = get_knowledge_copy(state, participant);

    let possible_claim = if let Some(knowledge) = possible_knowledge {
        let claim = if is_werewolf(knowledge.role) {
            //TODO better lying
            //equal probability of all plausible possibilities?
            attempt_not_to_be_picked(state, participant)
        } else if is_minion(knowledge.role) {
            if knowledge.known_werewolves.len() > 0 {
                //TODO try to cover for Werewolves and not specifically try to get picked?
                attempt_to_be_picked(state, participant)
            } else {
                //TODO try to get another player voted for
                Simple(Villager)
            }
        } else if is_tanner(knowledge.role) {
            attempt_to_be_picked(state, participant)
        } else {
            //TODO occasionally lying while a villager to try and snuff out werewolves
            knowledge.true_claim
        };

        Some(claim)
    } else {
        None
    };

    if let Some(claim) = possible_claim {
        insert_claim(state, participant, claim);
    }
}

fn plausable_lie(state: &mut State, participant: Participant) -> Option<Claim> {
    let other_participants = get_other_participants(state, participant);

    let role_vec: Vec<Role> = state.role_spec.get_role_vector();

    let rng = &mut state.rng;
    //TODO more types of lies
    get_fake_robber_claim(&other_participants, &role_vec, rng).or_else(|| {
        get_fake_insomniac_claim(&role_vec, rng)
    })
}

//TODO should this and attempt_not_to_be_picked be so similar?
//maybe attempt_to_be_picked should look at prior claims and try to create confusion
//and the other one should try to reduce it?
fn attempt_to_be_picked(state: &mut State, participant: Participant) -> Claim {
    plausable_lie(state, participant).unwrap_or(Simple(Werewolf))
}

fn attempt_not_to_be_picked(state: &mut State, participant: Participant) -> Claim {
    plausable_lie(state, participant).unwrap_or(Simple(Villager))
}

fn get_fake_robber_claim<R: Rng>(other_participants: &Vec<Participant>,
                                 role_vec: &Vec<Role>,
                                 rng: &mut R)
                                 -> Option<Claim> {
    if !role_vec.contains(&Robber) {
        return None;
    }

    let filtered = role_vec.iter()
        .filter(|&&r| is_on_village_team(r) && !is_mason(r) && r != Robber)
        .map(|&r| r)
        .collect();

    if let (Some(&target), Some(&claimed_role)) =
        (get_random(&other_participants, rng), get_random(&filtered, rng)) {
        Some(RobberAction(target, claimed_role))
    } else {
        None
    }
}

fn get_fake_insomniac_claim<R: Rng>(role_vec: &Vec<Role>, rng: &mut R) -> Option<Claim> {
    if !role_vec.contains(&Insomniac) {
        return None;
    }

    if role_vec.contains(&Troublemaker) {
        let filtered = if role_vec.contains(&Robber) {
            role_vec.iter()
                .filter(|&&r| is_on_village_team(r) && !is_mason(r))
                .map(|&r| r)
                .collect()
        } else {
            role_vec.iter()
                .filter(|&&r| is_on_village_team(r) && !is_mason(r) && r != Troublemaker)
                .map(|&r| r)
                .collect()

        };

        if let Some(&claimed_role) = get_random(&filtered, rng) {
            Some(InsomniacAction(claimed_role))
        } else {
            //TODO cnfirm that expansions make a none case (and thus the return type) necessary
            None
        }
    } else if role_vec.contains(&Robber) {
        Some(InsomniacAction(Robber))
    } else {
        Some(InsomniacAction(Insomniac))
    }
}

fn get_random<'a, T, R: Rng>(things: &'a Vec<T>, rng: &mut R) -> Option<&'a T> {
    let len = things.len();

    if len > 0 {
        things.get(rng.gen_range(0, len)).map(|t| t)
    } else {
        None
    }
}

enum ClaimOrSilence {
    ActualClaim(Claim),
    Silence,
}
use ClaimOrSilence::*;

fn get_player_claim_or_silence(platform: &Platform,
                               state: &mut State,
                               left_mouse_pressed: bool,
                               left_mouse_released: bool)
                               -> Option<ClaimOrSilence> {

    let silence_spec = ButtonSpec {
        x: 12,
        y: 0,
        w: 20,
        h: 3,
        text: "Remain Silent".to_string(),
        id: 60,
    };

    if do_button(platform,
                 &mut state.ui_context,
                 &silence_spec,
                 left_mouse_pressed,
                 left_mouse_released) {
        return Some(Silence);
    }

    //TODO allow player to make claim
    None
}
fn insert_claim(state: &mut State, participant: Participant, claim: Claim) {
    //TODO update cpu_knowledge
    //for example, if someone claims to be the seer and their claim about
    //who someone is matches what you know, then most likely they are the
    //seer, and didn't just guess luckily

    state.claims.insert(participant, claim);
}
fn get_claim_vec(state: &State) -> Vec<(Participant, Claim)> {
    let mut result: Vec<(Participant, Claim)> = state.claims
        .iter()
        .map(|(&p, &c)| (p, c))
        .collect();

    result.sort();

    result
}

fn claims_to_lines(state: &State, claims: Vec<(Participant, Claim)>) -> Vec<String> {
    let mut result = Vec::new();

    for pair in claims.iter() {
        push_claim_lines(state, &mut result, pair);
        result.push("".to_string());
    }

    result
}

fn push_claim_lines(state: &State,
                    result: &mut Vec<String>,
                    &(participant, claim): &(Participant, Claim)) {

    if participant == Player {
        match claim {
            Simple(role) => {
                result.push(format!("You claim that you are {}", role));

                if role == Minion {
                    let possible_list = get_knowledge(state, participant).and_then(|k| {
                        let list = str_list(&k.known_werewolves.iter().collect());

                        if list.len() > 0 { Some(list) } else { None }
                    });
                    if let Some(list) = possible_list {
                        result.push(format!("and you know {} are werewolves.", list));
                    }

                }
            }
            DoppelSimple(doppel_target, role) => {
                result.push(format!("You claim you copied {}.", doppel_target));
                push_claim_lines(state, result, &(participant, Simple(role)));
            }
            MasonAction(Two(other_mason1, other_mason2)) => {
                result.push(format!("You claim that you are {} and so is {} and {}",
                                    Mason,
                                    other_mason1,
                                    other_mason2));
            }
            MasonAction(One(other_mason)) => {
                result.push(format!("You claim that you are {} and so is {}", Mason, other_mason));
            }
            MasonAction(Zero) => {
                result.push(format!("You claim that you are {} but no one else is", Mason));
            }
            DoppelMasonAction(doppel_target, other_masons) => {
                result.push(format!("You claim you copied {}.", doppel_target));
                push_claim_lines(state, result, &(participant, MasonAction(other_masons)));
            }
            RobberAction(p, role) => {
                result.push(format!("You claim that you are {}", Robber));
                result.push(format!("and you swapped roles with {} and they were {}", p, role));
            }
            DoppelRobberAction(doppel_target, p, role) => {
                result.push(format!("You claim you copied {}.", doppel_target));
                push_claim_lines(state, result, &(participant, RobberAction(p, role)));
            }
            SeerRevealOneAction(p, role) => {
                result.push(format!("You claim that you are {}", Seer));
                result.push(format!("and you looked at {} and they were {}", p, role));
            }
            DoppelSeerRevealOneAction(doppel_target, p, role) => {
                result.push(format!("You claim you copied {}.", doppel_target));
                push_claim_lines(state, result, &(participant, SeerRevealOneAction(p, role)));
            }
            SeerRevealTwoAction(centerpair, role1, role2) => {
                result.push(format!("You claim that you are {}", Seer));
                let message = format!("and you looked at the {} cards and they were {} and {}",
                                      centerpair,
                                      role1,
                                      role2);
                result.push(message);
            }
            DoppelSeerRevealTwoAction(doppel_target, centerpair, role1, role2) => {
                result.push(format!("You claim you copied {}.", doppel_target));
                push_claim_lines(state,
                                 result,
                                 &(participant, SeerRevealTwoAction(centerpair, role1, role2)));
            }
            TroublemakerAction(p1, p2) => {
                result.push(format!("You claim that you are {}", Troublemaker));
                let message = format!("and you swapeed the roles of {} and {}.", p1, p2);
                result.push(message);
            }
            DoppelTroublemakerAction(doppel_target, p1, p2) => {
                result.push(format!("You claim you copied {}.", doppel_target));
                push_claim_lines(state, result, &(participant, TroublemakerAction(p1, p2)));
            }
            InsomniacAction(role) => {
                result.push(format!("You claim that you are {}", Insomniac));
                let message = format!("and you are now {}.", role);
                result.push(message);
            }
            DoppelInsomniacAction(doppel_target, role) => {
                result.push(format!("You claim you copied {}.", doppel_target));
                push_claim_lines(state, result, &(participant, InsomniacAction(role)));
            }
            DrunkAction(card) => {
                result.push(format!("You claim that you are {}", Drunk));
                let message = format!("and you swapped with the {} card.", card);
                result.push(message);
            }
            DoppelDrunkAction(doppel_target, card) => {
                result.push(format!("You claim you copied {}.", doppel_target));
                push_claim_lines(state, result, &(participant, DrunkAction(card)));
            }
        }
    } else {
        match claim {
            Simple(role) => {
                result.push(format!("{} claims that they are {}", participant, role));

                if role == Minion {
                    let possible_list = get_known_werevolves_str_list(state, participant);
                    if let Some(list) = possible_list {
                        result.push(format!("and they know {} are werewolves.", list));
                    }
                }
            }
            DoppelSimple(doppel_target, role) => {
                result.push(format!("{} claims they copied {}", participant, doppel_target));
                push_claim_lines(state, result, &(participant, Simple(role)));
            }
            MasonAction(Two(other_mason1, other_mason2)) => {

                result.push(format!("{} claims that they are {} and so are {} and {}",
                                    participant,
                                    Mason,

                                    other_mason1,
                                    other_mason2));
            }

            MasonAction(One(other_mason)) => {
                let verb_form = if other_mason == Player { "are" } else { "is" };
                result.push(format!("{} claims that they are {} and so {} {}",
                                    participant,
                                    Mason,
                                    verb_form,
                                    other_mason));
            }
            MasonAction(Zero) => {
                result.push(format!("{} claims that they are {} but no one else is",
                                    participant,
                                    Mason));
            }
            DoppelMasonAction(doppel_target, other_masons) => {
                result.push(format!("{} claims they copied {}", participant, doppel_target));
                push_claim_lines(state, result, &(participant, MasonAction(other_masons)));
            }
            RobberAction(p, role) => {
                result.push(format!("{} claims that they are {}", participant, Robber));
                let pronoun = if p == Player { "You" } else { "they" };
                result.push(format!("and they swapped roles with {} and {} were {}",
                                    p,
                                    pronoun,
                                    role));
            }
            DoppelRobberAction(doppel_target, p, role) => {
                result.push(format!("{} claims they copied {}", participant, doppel_target));
                push_claim_lines(state, result, &(participant, RobberAction(p, role)));
            }
            SeerRevealOneAction(p, role) => {
                result.push(format!("{} claims that they are {}", participant, Seer));
                let pronoun = if p == Player { "You" } else { "they" };
                result.push(format!("and they looked at {} and {} were {}", p, pronoun, role));
            }
            DoppelSeerRevealOneAction(doppel_target, p, role) => {
                result.push(format!("{} claims they copied {}", participant, doppel_target));
                push_claim_lines(state, result, &(participant, SeerRevealOneAction(p, role)));
            }
            SeerRevealTwoAction(centerpair, role1, role2) => {
                result.push(format!("{} claims that they are {},", participant, Seer));
                let message = format!("they looked at the {} cards,", centerpair);
                result.push(message);
                result.push(format!("and they were {} and {}", role1, role2));
            }
            DoppelSeerRevealTwoAction(doppel_target, centerpair, role1, role2) => {
                result.push(format!("{} claims they copied {}", participant, doppel_target));
                push_claim_lines(state,
                                 result,
                                 &(participant, SeerRevealTwoAction(centerpair, role1, role2)));
            }
            TroublemakerAction(p1, p2) => {
                result.push(format!("{} claims that they are {}", participant, Troublemaker));
                result.push("and they swapeed the roles of the following two players:".to_string());
                let message = format!("{} and {}.", p1, p2);
                result.push(message);
            }
            DoppelTroublemakerAction(doppel_target, p1, p2) => {
                result.push(format!("{} claims they copied {}", participant, doppel_target));
                push_claim_lines(state, result, &(participant, TroublemakerAction(p1, p2)));
            }
            InsomniacAction(role) => {
                result.push(format!("{} claims that they were {}", participant, Insomniac));
                let message = format!("and they are now ... {}.", role);
                result.push(message);
            }
            DoppelInsomniacAction(doppel_target, role) => {
                result.push(format!("{} claims they copied {}", participant, doppel_target));
                push_claim_lines(state, result, &(participant, InsomniacAction(role)));
            }
            DrunkAction(card) => {
                result.push(format!("{} claims that they are {}", participant, Drunk));
                let message = format!("and they swapped with the {} card.", card);
                result.push(message);
            }
            DoppelDrunkAction(doppel_target, card) => {
                result.push(format!("{} claims they copied {}", participant, doppel_target));
                push_claim_lines(state, result, &(participant, DrunkAction(card)));
            }
        }
    }
}

fn get_known_werevolves_str_list(state: &State, participant: Participant) -> Option<String> {
    get_knowledge(state, participant).and_then(|k| {
       let list = str_list(&k.known_werewolves.iter().collect());

       if list.len() > 0 { Some(list) } else { None }
   })
}

fn make_remaining_claims(state: &mut State) {
    let mut cpu_participants = get_other_participants(state, Player);
    state.rng.shuffle(&mut cpu_participants);
    for &participant in cpu_participants.iter() {
        if !state.claims.contains_key(&participant) {
            make_cpu_claim(state, participant);
        }
    }

}

use std::fmt::Write;
fn str_list<T: std::fmt::Display>(things: &Vec<T>) -> String {
    let len = things.len();
    if len == 0 {
        "".to_string()
    } else if len == 1 {
        format!("{}", things[0])
    } else if len == 2 {
        format!("{} and {}", things[0], things[1])
    } else {
        let mut result = "".to_string();

        for i in 0..len - 1 {
            if i == len - 2 {
                write!(&mut result, "{}, and {}", things[i], things[i + 1]).unwrap();
            } else {
                write!(&mut result, "{}, ", things[i]).unwrap();
            }
        }

        result
    }
}

fn count_votes(votes: &Vec<Participant>) -> Vec<Participant> {
    let mut counts = HashMap::new();

    for &vote in votes.iter() {
        let counter = counts.entry(vote).or_insert(0);
        *counter += 1;
    }

    let max_count = counts.values()
        .max()
        .map(|&c| c)
        .unwrap_or(0);

    if max_count > 1 {
        counts.iter()
            .filter(|&(_, &count)| count == max_count)
            .map(|(&p, _)| p)
            .collect()
    } else {
        Vec::new()
    }
}

fn is_werewolf(role: Role) -> bool {
    match role {
        Werewolf |
        DoppelWerewolf(_) => true,
        _ => false,
    }
}
fn is_mason(role: Role) -> bool {
    match role {
        Mason | DoppelMason(_) => true,
        _ => false,
    }
}
fn is_minion(role: Role) -> bool {
    match role {
        Minion | DoppelMinion(_) => true,
        _ => false,
    }
}
fn is_tanner(role: Role) -> bool {
    match role {
        Tanner | DoppelTanner(_) => true,
        _ => false,
    }
}
fn is_on_village_team(role: Role) -> bool {
    !(is_werewolf(role) || is_minion(role) || is_tanner(role))
}


fn swap_roles(state: &mut State, p1: Participant, p2: Participant) {
    unsafe {
        let ptr1 = get_role_ptr(state, p1);
        let ptr2 = get_role_ptr(state, p2);

        std::ptr::swap(ptr1, ptr2);
    }
}

unsafe fn get_role_ptr(state: &mut State, p: Participant) -> *mut Role {
    match p {
        Player => &mut state.player,
        Cpu(i) => &mut state.cpu_roles[i],
    }
}

fn swap_role_with_center(state: &mut State, p: Participant, center_card: CenterCard) {
    unsafe {
        let role_ptr = get_role_ptr(state, p);
        let center_ptr = get_center_role_ptr(state, center_card);

        std::ptr::swap(role_ptr, center_ptr);
    }
}

unsafe fn get_center_role_ptr(state: &mut State, center_card: CenterCard) -> *mut Role {
    match center_card {
        First => &mut state.cpu_roles[0],
        Second => &mut state.cpu_roles[1],
        Third => &mut state.cpu_roles[2],
    }
}

fn get_other_participants(state: &State, participant: Participant) -> Vec<Participant> {
    get_participants(state)
        .iter()
        .map(|&p| p)
        .filter(|&p| p != participant)
        .collect()
}

fn get_role(state: &State, participant: Participant) -> Option<Role> {
    match participant {
        Player => Some(state.player),
        Cpu(i) => state.cpu_roles.get(i).map(|&r| r),
    }
}

fn get_participants(state: &State) -> Vec<Participant> {
    let mut result = vec![Player];

    for i in 0..state.cpu_roles.len() {
        result.push(Cpu(i));
    }

    result
}

fn pick_cpu_player_or_skip(platform: &Platform,
                           state: &mut State,
                           left_mouse_pressed: bool,
                           left_mouse_released: bool)
                           -> ParticipantOrSkip {
    if let Some(p) = pick_cpu_player(platform, state, left_mouse_pressed, left_mouse_released) {
        Chosen(p)
    } else {

        if do_button(platform,
                     &mut state.ui_context,
                     &ButtonSpec {
                          x: 0,
                          y: 8,
                          w: 11,
                          h: 3,
                          text: "Skip".to_string(),
                          id: 11,
                      },
                     left_mouse_pressed,
                     left_mouse_released) {
            Skip
        } else {
            NoChoice
        }
    }



}

enum SeerChoice {
    SeerCpuOrSkip(ParticipantOrSkip),
    ChosenPair(CenterPair),
}
use SeerChoice::*;


fn pick_seer_choice(platform: &Platform,
                    state: &mut State,
                    left_mouse_pressed: bool,
                    left_mouse_released: bool)
                    -> SeerChoice {

    let all_pairs = CenterPair::all_values();
    for index in 0..all_pairs.len() {
        let i = index as i32;
        let pair = all_pairs[index];
        if do_button(platform,
                     &mut state.ui_context,
                     &ButtonSpec {
                          x: 0,
                          y: 12 + (i * 4),
                          w: 20,
                          h: 3,
                          text: pair.to_string(),
                          id: 72 + i,
                      },
                     left_mouse_pressed,
                     left_mouse_released) {
            return ChosenPair(pair);
        }

    }

    SeerCpuOrSkip(pick_cpu_player_or_skip(platform, state, left_mouse_pressed, left_mouse_released))
}

pub enum ParticipantOrSkip {
    Skip,
    Chosen(Participant),
    NoChoice,
}
use ParticipantOrSkip::*;

fn pick_cpu_player(platform: &Platform,
                   state: &mut State,
                   left_mouse_pressed: bool,
                   left_mouse_released: bool)
                   -> Option<Participant> {
    let cpu_participants = get_cpu_participants(state);

    pick_displayable(platform,
                     state,
                     left_mouse_pressed,
                     left_mouse_released,
                     &cpu_participants)
}

use std::fmt::Display;
//assumes the string representaion fits on one line
fn pick_displayable<T: Display + Copy>(platform: &Platform,
                                       state: &mut State,
                                       left_mouse_pressed: bool,
                                       left_mouse_released: bool,
                                       things: &Vec<T>)
                                       -> Option<T> {
    let size = (platform.size)();

    let mut strings: Vec<String> = things.iter().map(|t| t.to_string()).collect();

    //3 spaces on either side
    let width: i32 = 6 + strings.iter().fold(0, |acc, s| std::cmp::max(acc, s.len())) as i32;
    for i in (0..things.len()).rev() {
        let index = i as i32;

        if let Some(string) = strings.pop() {
            let spec = ButtonSpec {
                x: size.width - width,
                y: (index + 1) * 4,
                w: width,
                h: 3,
                text: string,
                id: 12 + index,
            };

            if do_button(platform,
                         &mut state.ui_context,
                         &spec,
                         left_mouse_pressed,
                         left_mouse_released) {
                return Some(things[i]);
            }
        }

    }

    None
}


fn get_cpu_participants(state: &State) -> Vec<Participant> {
    let mut result = Vec::new();

    for i in 0..state.cpu_roles.len() {
        result.push(Cpu(i));
    }

    result
}

fn get_vote(participant: Participant,
            participants: Vec<Participant>,
            knowledge: &Knowledge,
            rng: &mut StdRng)
            -> Participant {
    //TODO count the number of valid ways that a given set of claims being true would be consistent
    //given known facts and assume those claims are true
    let filtered: Vec<Participant> = if is_werewolf(knowledge.role) || knowledge.role == Minion {
        let mut vec: Vec<Participant> = knowledge.known_villagers
            .iter()
            .map(|&p| p)
            .collect();
        vec.sort();
        if let Some(&villager) = rng.choose(&vec) {
            return villager;
        }

        participants.iter()
            .filter(|p| **p != participant && !knowledge.known_werewolves.contains(p))
            .map(|&p| p)
            .collect()


    } else {
        let mut vec: Vec<Participant> = knowledge.known_werewolves
            .iter()
            .map(|&p| p)
            .collect();
        vec.sort();
        if let Some(&werewolf) = rng.choose(&vec) {
            return werewolf;
        }

        participants.iter()
            .filter(|p| **p != participant && !knowledge.known_villagers.contains(p))
            .map(|&p| p)
            .collect()
    };

    if let Some(&p) = rng.choose(&filtered) {
        return p;
    }

    //TODO do process of elimination with known_non_active. (can this happen earlier?)

    //vote clockwise
    println!("clockwise : {}", participant);
    *(match participant {
              Player => participants.get(0),
              Cpu(i) => participants.get(i + 1),
          })
         .unwrap_or(&Player)
}

fn get_werewolves(state: &State) -> Vec<Participant> {
    let mut result = Vec::new();

    if is_werewolf(state.player) {
        result.push(Player);
    }

    for i in 0..state.cpu_roles.len() {
        if is_werewolf(state.cpu_roles[i]) {
            result.push(Cpu(i));
        }
    }

    result
}

fn get_masons(state: &State) -> Vec<Participant> {
    let mut result = Vec::new();

    if is_mason(state.player) {
        result.push(Player);
    }

    for i in 0..state.cpu_roles.len() {
        if is_mason(state.cpu_roles[i]) {
            result.push(Cpu(i));
        }
    }

    result
}

fn ready_button(platform: &Platform,
                state: &mut State,
                left_mouse_pressed: bool,
                left_mouse_released: bool)
                -> bool {
    let size = (platform.size)();
    let ready_spec = ButtonSpec {
        x: (size.width / 2) - 6,
        y: size.height - 4,
        w: 11,
        h: 3,
        text: "Ready".to_string(),
        id: 2,
    };

    do_button(platform,
              &mut state.ui_context,
              &ready_spec,
              left_mouse_pressed,
              left_mouse_released)

}

fn cross_mode_event_handling(platform: &Platform, state: &mut State, event: &Event) {
    match *event {
        Event::KeyPressed { key: KeyCode::R, ctrl: true, shift: _ } => {
            println!("reset");
            *state = new_state((platform.size)());
        }
        _ => (),
    }
}

fn draw(platform: &Platform, state: &State) {}

pub struct ButtonSpec {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub text: String,
    pub id: i32,
}

//calling this once will swallow multiple clicks on the button. We could either
//pass in and return the number of clicks to fix that, or this could simply be
//called multiple times per frame (once for each click).
fn do_button(platform: &Platform,
             context: &mut UIContext,
             spec: &ButtonSpec,
             left_mouse_pressed: bool,
             left_mouse_released: bool)
             -> bool {
    let mut result = false;

    let mouse_pos = (platform.mouse_position)();
    let inside = inside_rect(mouse_pos, spec.x, spec.y, spec.w, spec.h);
    let id = spec.id;

    if context.active == id {
        if left_mouse_released {
            result = context.hot == id && inside;

            context.set_not_active();
        }
    } else if context.hot == id {
        if left_mouse_pressed {
            context.set_active(id);
        }
    }

    if inside {
        context.set_next_hot(id);
    }

    if context.active == id && (platform.key_pressed)(KeyCode::MouseLeft) {
        draw_rect_with(platform,
                       spec.x,
                       spec.y,
                       spec.w,
                       spec.h,
                       ["╔", "═", "╕", "║", "│", "╙", "─", "┘"]);
    } else if context.hot == id {
        draw_rect_with(platform,
                       spec.x,
                       spec.y,
                       spec.w,
                       spec.h,
                       ["┌", "─", "╖", "│", "║", "╘", "═", "╝"]);
    } else {
        draw_rect(platform, spec.x, spec.y, spec.w, spec.h);
    }

    print_centered_line(platform, spec.x, spec.y, spec.w, spec.h, &spec.text);

    return result;
}

pub fn inside_rect(point: Point, x: i32, y: i32, w: i32, h: i32) -> bool {
    x <= point.x && y <= point.y && point.x < x + w && point.y < y + h
}

fn print_centered_line(platform: &Platform, x: i32, y: i32, w: i32, h: i32, text: &str) {
    let x_ = {
        let rect_middle = x + (w / 2);

        std::cmp::max(rect_middle - (text.chars().count() as f32 / 2.0) as i32, 0)
    };

    let y_ = y + (h / 2);

    (platform.print_xy)(x_, y_, &text);
}


fn draw_rect(platform: &Platform, x: i32, y: i32, w: i32, h: i32) {
    draw_rect_with(platform,
                   x,
                   y,
                   w,
                   h,
                   ["┌", "─", "┐", "│", "│", "└", "─", "┘"]);
}

fn draw_double_line_rect(platform: &Platform, x: i32, y: i32, w: i32, h: i32) {
    draw_rect_with(platform,
                   x,
                   y,
                   w,
                   h,
                   ["╔", "═", "╗", "║", "║", "╚", "═", "╝"]);
}

fn draw_rect_with(platform: &Platform, x: i32, y: i32, w: i32, h: i32, edges: [&str; 8]) {
    (platform.clear)(Some(Rect::from_values(x, y, w, h)));

    let right = x + w - 1;
    let bottom = y + h - 1;
    // top
    (platform.print_xy)(x, y, edges[0]);
    for i in (x + 1)..right {
        (platform.print_xy)(i, y, edges[1]);
    }
    (platform.print_xy)(right, y, edges[2]);

    // sides
    for i in (y + 1)..bottom {
        (platform.print_xy)(x, i, edges[3]);
        (platform.print_xy)(right, i, edges[4]);
    }

    //bottom
    (platform.print_xy)(x, bottom, edges[5]);
    for i in (x + 1)..right {
        (platform.print_xy)(i, bottom, edges[6]);
    }
    (platform.print_xy)(right, bottom, edges[7]);
}
