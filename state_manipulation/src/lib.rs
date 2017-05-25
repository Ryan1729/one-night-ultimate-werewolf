extern crate rand;
extern crate common;

use common::*;
use common::Role::*;
use common::Turn::*;
use common::Participant::*;
use common::Claim::*;
use common::CenterPair::*;
use common::CenterCard::*;

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
    let (player, cpu_roles, table_roles, player_knowledge, cpu_knowledge, _) =
        get_roles_and_knowledge(&mut rng);

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
    }
}

fn get_roles_and_knowledge(rng: &mut StdRng)
                           -> (Role, Vec<Role>, [Role; 3], Knowledge, Vec<Knowledge>, bool) {
    //DoppelVillager(Player) represents the Doppelganger card
    let mut roles = vec![Werewolf, Minion, DoppelVillager(Player), Troublemaker, Werewolf, Seer];

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

    let reverse_spec = ButtonSpec {
        x: 0,
        y: 0,
        w: 10,
        h: 3,
        text: "Next".to_string(),
        id: 1,
    };

    if do_button(platform,
                 &mut state.ui_context,
                 &reverse_spec,
                 left_mouse_pressed,
                 left_mouse_released) {
        state.turn = state.turn.next();
    }
    let t = state.turn;
    match state.turn {
        Ready => {
            (platform.print_xy)(10, 12, "Ready to start a game?");

            //TODO pick roles and number of players

            if ready_button(platform, state, left_mouse_pressed, left_mouse_released) {
                let (player,
                     cpu_roles,
                     table_roles,
                     player_knowledge,
                     cpu_knowledge,
                     player_is_doppel) = get_roles_and_knowledge(&mut state.rng);

                state.player = player;
                state.initial_player = player;
                state.initial_cpu_roles = cpu_roles.to_owned();
                state.cpu_roles = cpu_roles;
                state.table_roles = table_roles;
                state.player_knowledge = player_knowledge;
                state.cpu_knowledge = cpu_knowledge;

                state.turn = SeeRole(player_is_doppel);
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
                (platform.print_xy)(10, 12, &format!("You are {}.", state.player));

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
            let werewolves = get_werewolves(state);

            if state.initial_player == Minion {
                (platform.print_xy)(10,
                                    10,
                                    "Minion, wake up. Werewolves, stick out
your thumb so the Minion can see who you are.");

                list_werewolves(platform, &werewolves);

                if ready_button(platform, state, left_mouse_pressed, left_mouse_released) {
                    state.turn = state.turn.next();
                }
            } else {
                if let Some(minion_index) = linear_search(&state.initial_cpu_roles, &Minion) {
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
                        DoppelMasonAction(p, other_masons.pop())
                    } else {
                        MasonAction(other_masons.pop())
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
            if state.initial_player == Troublemaker {
                (platform.print_xy)(15,
                                    3,
                                    "Troublemaker, wake up.
 You may exchange cards between two other players.");

                (platform.print_xy)(15, 5, "Choose the first other player:");


                let choice = pick_cpu_player_or_skip(platform,
                                                     state,
                                                     left_mouse_pressed,
                                                     left_mouse_released);
                match choice {
                    Skip => {
                        state.turn = state.turn.next();
                    }
                    Chosen(chosen) => {
                        state.turn = TroublemakerSecondChoice(chosen);
                    }
                    NoChoice => {}
                }
            } else {
                if let Some(troublemaker_index) =
                    linear_search(&state.initial_cpu_roles, &Troublemaker) {
                    let troublemaker = Cpu(troublemaker_index);

                    let mut other_participants = get_other_participants(state, troublemaker);
                    state.rng.shuffle(&mut other_participants);

                    if let (Some(first_choice), Some(second_choice)) =
                        (other_participants.pop(), other_participants.pop()) {
                        swap_roles(state, first_choice, second_choice);

                        if let Some(knowledge) = get_knowledge_mut(state, troublemaker) {
                            knowledge.true_claim = TroublemakerAction(first_choice, second_choice);
                            knowledge.troublemaker_swap = Some((first_choice, second_choice));
                        }
                    }
                }

                state.turn = state.turn.next();
            };
        }
        TroublemakerSecondChoice(first_choice) => {
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
                state.player_knowledge.true_claim = TroublemakerAction(first_choice, second_choice);

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
                state.turn = TroublemakerTurn;
            }
        }
        DrunkTurn => {
            if state.initial_player == Drunk {
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

                        if let Some(knowledge) = get_knowledge_mut(state, Player) {
                            knowledge.true_claim = DrunkAction(chosen);
                            knowledge.drunk_swap = Some((Player, chosen));
                        }

                        state.turn = state.turn.next();
                    }
                    None => {}
                }
            } else {
                if let Some(drunk_index) = linear_search(&state.initial_cpu_roles, &Drunk) {
                    let drunk = Cpu(drunk_index);

                    let target = state.rng.gen::<CenterCard>();

                    swap_role_with_center(state, drunk, target);

                    if let Some(knowledge) = get_knowledge_mut(state, drunk) {
                        knowledge.true_claim = DrunkAction(target);
                        knowledge.drunk_swap = Some((drunk, target));
                    }
                }

                state.turn = state.turn.next();
            };
        }
        InsomniacTurn => {
            if state.initial_player == Insomniac {
                (platform.print_xy)(15, 3, "Insomniac, wake up and look at your card.");

                (platform.print_xy)(15, 5, &format!("You are {}", state.player));
                state.player_knowledge.true_claim = InsomniacAction(state.player);

                if ready_button(platform, state, left_mouse_pressed, left_mouse_released) {
                    state.turn = state.turn.next();
                }
            } else {
                if let Some(i) = linear_search(&state.initial_cpu_roles, &Insomniac) {
                    let knowledge = &mut state.cpu_knowledge[i];
                    knowledge.insomniac_peek = true;

                    if let Some(&role) = state.cpu_roles.get(i) {
                        knowledge.true_claim = InsomniacAction(role);
                        knowledge.role = role;
                    }
                }

                state.turn = state.turn.next();
            }
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

            for i in 0..claims.len() {
                let index = i as i32;

                display_claim(state,
                              platform,
                              10,
                              6 + (index * MAX_CLAIM_HEIGHT),
                              &claims[i]);
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

            (platform.print_xy)(10, 20, &format!("You are {}", state.player));

            for i in 0..state.cpu_roles.len() {
                (platform.print_xy)(10,
                                    21 + i as i32,
                                    &format!("{} is {}", Cpu(i), state.cpu_roles[i]));
            }

            if ready_button(platform, state, left_mouse_pressed, left_mouse_released) {
                state.turn = state.turn.next();
            }
        }

    }

    if t != state.turn {
        println!("{:?}", state.turn);
    }

    draw(platform, state);

    false
}

fn robber_action(state: &State, participant: Participant, p: Participant, r: Role) -> Claim {
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
            println!("{}", seer);

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
fn make_cpu_claim(state: &mut State, participant: Participant) {
    if participant == Player {
        return;
    }

    let possible_claim = if let Some(knowledge) = get_knowledge(state, participant) {
        let claim = if is_werewolf(knowledge.role) {
            //TODO better lying
            //equal probability of all plausible possibilities?
            Simple(Villager)
        } else if is_minion(knowledge.role) {
            if knowledge.known_werewolves.len() > 0 {
                //TODO look at already made claims and try to cover for Werewolves?
                Simple(Werewolf)
            } else {
                //TODO try to get another player voted for
                Simple(Villager)
            }
        } else if is_tanner(knowledge.role) {
            //TODO try to get self voted for more convincingly
            Simple(Werewolf)
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
fn display_claim(state: &State,
                 platform: &Platform,
                 x: i32,
                 y: i32,
                 &(participant, claim): &(Participant, Claim)) {
    if participant == Player {
        match claim {
            Simple(role) => {
                (platform.print_xy)(x, y, &format!("You claim that you are {}", role));

                if role == Minion {
                    let possible_list = get_knowledge(state, participant).and_then(|k| {
                        let list = str_list(&k.known_werewolves.iter().collect());

                        if list.len() > 0 { Some(list) } else { None }
                    });
                    if let Some(list) = possible_list {

                        (platform.print_xy)(x,

                                            y + 1,
                                            &format!("and you know {} are werewolves.", list));
                    }

                }
            }
            DoppelSimple(doppel_target, role) => {
                (platform.print_xy)(x, y, &format!("You claim you copied {}.", doppel_target));
                display_claim(state, platform, x, y + 1, &(participant, Simple(role)));
            }
            MasonAction(Some(other_mason)) => {
                (platform.print_xy)(x,
                                    y,
                                    &format!("You claim that you are {} and so is {}",
                                             Mason,
                                             other_mason));
            }
            DoppelMasonAction(doppel_target, Some(other_mason)) => {
                (platform.print_xy)(x, y, &format!("You claim you copied {}.", doppel_target));
                display_claim(state,
                              platform,
                              x,
                              y + 1,
                              &(participant, MasonAction(Some(other_mason))));
            }
            MasonAction(None) => {
                (platform.print_xy)(x,
                                    y,
                                    &format!("You claim that you are {} but no one else is",
                                             Mason));
            }
            DoppelMasonAction(doppel_target, None) => {
                (platform.print_xy)(x, y, &format!("You claim you copied {}.", doppel_target));
                display_claim(state, platform, x, y + 1, &(participant, MasonAction(None)));
            }
            RobberAction(p, role) => {
                (platform.print_xy)(x, y, &format!("You claim that you are {}", Robber));
                (platform.print_xy)(x,
                                    y + 1,
                                    &format!("and you swapped roles with {} and they were {}",
                                             p,
                                             role));
            }
            DoppelRobberAction(doppel_target, p, role) => {
                (platform.print_xy)(x, y, &format!("You claim you copied {}.", doppel_target));
                display_claim(state,
                              platform,
                              x,
                              y + 1,
                              &(participant, RobberAction(p, role)));
            }
            SeerRevealOneAction(p, role) => {
                (platform.print_xy)(x, y, &format!("You claim that you are {}", Seer));
                (platform.print_xy)(x,
                                    y + 1,
                                    &format!("and you looked at {} and they were {}", p, role));
            }
            DoppelSeerRevealOneAction(doppel_target, p, role) => {
                (platform.print_xy)(x, y, &format!("You claim you copied {}.", doppel_target));
                display_claim(state,
                              platform,
                              x,
                              y + 1,
                              &(participant, SeerRevealOneAction(p, role)));
            }
            SeerRevealTwoAction(centerpair, role1, role2) => {
                (platform.print_xy)(x, y, &format!("You claim that you are {}", Seer));
                let message = &format!("and you looked at the {} cards and they were {} and {}",
                                       centerpair,
                                       role1,
                                       role2);
                (platform.print_xy)(x, y + 1, message);
            }
            DoppelSeerRevealTwoAction(doppel_target, centerpair, role1, role2) => {
                (platform.print_xy)(x, y, &format!("You claim you copied {}.", doppel_target));
                display_claim(state,
                              platform,
                              x,
                              y + 1,
                              &(participant, SeerRevealTwoAction(centerpair, role1, role2)));
            }
            TroublemakerAction(p1, p2) => {
                (platform.print_xy)(x, y, &format!("You claim that you are {}", Troublemaker));
                let message = &format!("and you swapeed the roles of {} and {}.", p1, p2);
                (platform.print_xy)(x, y + 1, message);
            }
            DoppelTroublemakerAction(doppel_target, p1, p2) => {
                (platform.print_xy)(x, y, &format!("You claim you copied {}.", doppel_target));
                display_claim(state,
                              platform,
                              x,
                              y + 1,
                              &(participant, TroublemakerAction(p1, p2)));
            }
            InsomniacAction(role) => {
                (platform.print_xy)(x, y, &format!("You claim that you are {}", Insomniac));
                let message = &format!("and you are now {}.", role);
                (platform.print_xy)(x, y + 1, message);
            }
            DoppelInsomniacAction(doppel_target, role) => {
                (platform.print_xy)(x, y, &format!("You claim you copied {}.", doppel_target));
                display_claim(state,
                              platform,
                              x,
                              y + 1,
                              &(participant, InsomniacAction(role)));
            }
            DrunkAction(card) => {
                (platform.print_xy)(x, y, &format!("You claim that you are {}", Drunk));
                let message = &format!("and you swapped with the {} card.", card);
                (platform.print_xy)(x, y + 1, message);
            }
            DoppelDrunkAction(doppel_target, card) => {
                (platform.print_xy)(x, y, &format!("You claim you copied {}.", doppel_target));
                display_claim(state, platform, x, y + 1, &(participant, DrunkAction(card)));
            }
        }
    } else {
        match claim {
            Simple(role) => {
                (platform.print_xy)(x,
                                    y,
                                    &format!("{} claims that they are {}", participant, role));

                if role == Minion {
                    let possible_list = get_known_werevolves_str_list(state, participant);
                    if let Some(list) = possible_list {
                        (platform.print_xy)(x,
                                            y + 1,
                                            &format!("and they know {} are werewolves.", list));
                    }
                }
            }
            DoppelSimple(doppel_target, role) => {
                (platform.print_xy)(x,
                                    y,
                                    &format!("{} claims they copied {}",
                                             participant,
                                             doppel_target));
                display_claim(state, platform, x, y + 1, &(participant, Simple(role)));
            }
            MasonAction(Some(other_mason)) => {
                let verb_form = if other_mason == Player { "are" } else { "is" };
                (platform.print_xy)(x,
                                    y,
                                    &format!("{} claims that they are {} and so {} {}",
                                             participant,
                                             Mason,
                                             verb_form,
                                             other_mason));
            }
            DoppelMasonAction(doppel_target, Some(other_mason)) => {
                (platform.print_xy)(x,
                                    y,
                                    &format!("{} claims they copied {}",
                                             participant,
                                             doppel_target));
                display_claim(state,
                              platform,
                              x,
                              y + 1,
                              &(participant, MasonAction(Some(other_mason))));
            }
            MasonAction(None) => {
                (platform.print_xy)(x,
                                    y,
                                    &format!("{} claims that they are {} but no one else is",
                                             participant,
                                             Mason));
            }
            DoppelMasonAction(doppel_target, None) => {
                (platform.print_xy)(x,
                                    y,
                                    &format!("{} claims they copied {}",
                                             participant,
                                             doppel_target));
                display_claim(state, platform, x, y + 1, &(participant, MasonAction(None)));
            }
            RobberAction(p, role) => {
                (platform.print_xy)(x,
                                    y,
                                    &format!("{} claims that they are {}", participant, Robber));
                let pronoun = if p == Player { "You" } else { "they" };
                (platform.print_xy)(x,
                                    y + 1,
                                    &format!("and they swapped roles with {} and {} were {}",
                                             p,
                                             pronoun,
                                             role));
            }
            DoppelRobberAction(doppel_target, p, role) => {
                (platform.print_xy)(x,
                                    y,
                                    &format!("{} claims they copied {}",
                                             participant,
                                             doppel_target));
                display_claim(state,
                              platform,
                              x,
                              y + 1,
                              &(participant, RobberAction(p, role)));
            }
            SeerRevealOneAction(p, role) => {
                (platform.print_xy)(x,
                                    y,
                                    &format!("{} claims that they are {}", participant, Seer));
                let pronoun = if p == Player { "You" } else { "they" };
                (platform.print_xy)(x,
                                    y + 1,
                                    &format!("and they looked at {} and {} were {}",
                                             p,
                                             pronoun,
                                             role));
            }
            DoppelSeerRevealOneAction(doppel_target, p, role) => {
                (platform.print_xy)(x,
                                    y,
                                    &format!("{} claims they copied {}",
                                             participant,
                                             doppel_target));
                display_claim(state,
                              platform,
                              x,
                              y + 1,
                              &(participant, SeerRevealOneAction(p, role)));
            }
            SeerRevealTwoAction(centerpair, role1, role2) => {
                (platform.print_xy)(x,
                                    y,
                                    &format!("{} claims that they are {}", participant, Seer));
                let message = &format!("and they looked at the {} cards and they were {} and {}",
                                       centerpair,
                                       role1,
                                       role2);
                (platform.print_xy)(x, y + 1, message);
            }
            DoppelSeerRevealTwoAction(doppel_target, centerpair, role1, role2) => {
                (platform.print_xy)(x,
                                    y,
                                    &format!("{} claims they copied {}",
                                             participant,
                                             doppel_target));
                display_claim(state,
                              platform,
                              x,
                              y + 1,
                              &(participant, SeerRevealTwoAction(centerpair, role1, role2)));
            }
            TroublemakerAction(p1, p2) => {
                (platform.print_xy)(x,
                                    y,
                                    &format!("{} claims that they are {}",
                                             participant,
                                             Troublemaker));
                let message = &format!("and they swapeed the roles of the following two players:
    {} and {}.",
                                       p1,
                                       p2);
                (platform.print_xy)(x, y + 1, message);
            }
            DoppelTroublemakerAction(doppel_target, p1, p2) => {
                (platform.print_xy)(x,
                                    y,
                                    &format!("{} claims they copied {}",
                                             participant,
                                             doppel_target));
                display_claim(state,
                              platform,
                              x,
                              y + 1,
                              &(participant, TroublemakerAction(p1, p2)));
            }
            InsomniacAction(role) => {
                (platform.print_xy)(x,
                                    y,
                                    &format!("{} claim that they are {}", participant, Insomniac));
                let message = &format!("and they are now {}.", role);
                (platform.print_xy)(x, y + 1, message);
            }
            DoppelInsomniacAction(doppel_target, role) => {
                (platform.print_xy)(x,
                                    y,
                                    &format!("{} claims they copied {}",
                                             participant,
                                             doppel_target));
                display_claim(state,
                              platform,
                              x,
                              y + 1,
                              &(participant, InsomniacAction(role)));
            }
            DrunkAction(card) => {
                (platform.print_xy)(x,
                                    y,
                                    &format!("{} claim that they are {}", participant, Drunk));
                let message = &format!("and they swapped with the {} card.", card);
                (platform.print_xy)(x, y + 1, message);
            }
            DoppelDrunkAction(doppel_target, card) => {
                (platform.print_xy)(x,
                                    y,
                                    &format!("{} claims they copied {}",
                                             participant,
                                             doppel_target));
                display_claim(state, platform, x, y + 1, &(participant, DrunkAction(card)));
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
        Minion
        // | DoppelMinion(_)
        => true,
        _ => false,
    }
}
fn is_tanner(role: Role) -> bool {
    match role {
        Tanner | DoppelTanner(_) => true,
        _ => false,
    }
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

    let width: usize = strings.iter().fold(0, |acc, s| std::cmp::max(acc, s.len()));
    // println!("{}", width);
    for i in (0..things.len()).rev() {
        let index = i as i32;

        if let Some(string) = strings.pop() {
            let spec = ButtonSpec {
                x: (size.width / 2) - 6,
                y: (index + 2) * 4,
                w: (width as i32) + 6,
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

        rect_middle - (text.chars().count() as f32 / 2.0) as i32
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
