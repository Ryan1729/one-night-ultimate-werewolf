extern crate rand;
extern crate common;

use common::*;
use common::Role::*;
use common::Turn::*;
use common::Participant::*;

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
    let mut roles = vec![Werewolf, Werewolf, Robber, Villager, Villager, Villager];

    rng.shuffle(&mut roles);

    let player = roles.pop().unwrap();

    let table_roles = [roles.pop().unwrap(), roles.pop().unwrap(), roles.pop().unwrap()];

    let cpu_roles = roles;

    let cpu_knowledge = cpu_roles.iter().map(|&role| Knowledge::new(role)).collect();

    State {
        rng: rng,
        title_screen: title_screen,
        player,
        cpu_roles,
        table_roles,
        turn: Ready,
        player_knowledge: Knowledge::new(player),
        cpu_knowledge,
        votes: Vec::new(),
        claims: HashMap::new(),
        ui_context: UIContext::new(),
    }
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
                state.turn = state.turn.next();
            };
        }
        SeeRole => {
            (platform.print_xy)(10, 12, &format!("You are a {}.", state.player));

            if ready_button(platform, state, left_mouse_pressed, left_mouse_released) {
                state.turn = state.turn.next();
            };
        }
        Werewolves => {
            let werewolves = get_werewolves(state);

            let ready = if state.player == Werewolf {
                (platform.print_xy)(10, 10, "Werewolves, wake up and look for other werewolves.");

                for i in 0..werewolves.len() {
                    let index = i as i32;

                    match werewolves[i] {
                        Player => (platform.print_xy)(10, 12 + index, "You are a werewolf. (duh!)"),
                        cpu => {
                            (platform.print_xy)(10, 12 + index, &format!("{} is a werewolf.", cpu))
                        }
                    }
                }

                ready_button(platform, state, left_mouse_pressed, left_mouse_released)
            } else {
                true
            };

            if ready {
                for &werewolf in werewolves.iter() {
                    match werewolf {
                        Player => {
                            state.player_knowledge.known_werewolves.extend_from_slice(&werewolves);
                        }
                        Cpu(index) => {
                            let ref mut knowledge = state.cpu_knowledge[index];

                            knowledge.known_werewolves.extend_from_slice(&werewolves);
                        }
                    }
                }

                state.turn = state.turn.next();
            }

        }
        RobberTurn => {
            if state.player == Robber {
                (platform.print_xy)(15,
                                    3,
                                    "Robber, wake up.
You may exchange your card with another player’s card,
and then view your new card.");


                let possible_chosen = pick_cpu_player_or_skip(platform,
                                                              state,
                                                              left_mouse_pressed,
                                                              left_mouse_released);
                match possible_chosen {
                    Skip => {
                        state.turn = state.turn.next();
                    }
                    Chosen(chosen) => {
                        swap_roles(state, Player, chosen);

                        state.turn = RobberReveal;
                    }
                    NoChoice => {}
                }
            } else {
                if let Some(robber_index) = linear_search(&state.cpu_roles, &Robber) {
                    let robber = Cpu(robber_index);

                    let other_participants = get_other_participants(state, robber);
                    if let Some(&chosen) = state.rng.choose(&other_participants) {
                        swap_roles(state, robber, chosen);
                    }
                }

                state.turn = state.turn.next();
            };
        }
        RobberReveal => {
            (platform.print_xy)(10, 10, &format!("You are now a {}.", state.player));

            if ready_button(platform, state, left_mouse_pressed, left_mouse_released) {
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
                for i in 0..first_speaker_count {
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

                display_claim(platform, 10, 6 + (index * MAX_CLAIM_HEIGHT), &claims[i]);
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
            let targets = count_votes(&just_votes);

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

                if hit_werevoles_count >= 1 {
                    if hit_werevoles_count == 1 {
                        (platform.print_xy)(10, 12, "A werewolf died!");
                    } else {
                        (platform.print_xy)(10,
                                            12,
                                            &format!("{} werewolves died!", hit_werevoles_count));
                    }
                    (platform.print_xy)(10, 13, "Village team wins!");
                } else {
                    let werewolves = get_werewolves(state);

                    if werewolves.len() > 0 {
                        (platform.print_xy)(10,
                                            12,
                                            "No werewolves died but a player was a werewolf!");
                        (platform.print_xy)(10, 13, "Werewolf team wins!");
                    } else {
                        (platform.print_xy)(10,
                                            12,
                                            "No werewolves died but a nobody was a werewolf!");
                        (platform.print_xy)(10, 13, "Nobody wins!");
                    }
                }
            }


            (platform.print_xy)(10, 20, &format!("You are a {}", state.player));

            for i in 0..state.cpu_roles.len() {
                (platform.print_xy)(10,
                                    21 + i as i32,
                                    &format!("{} is a {}", Cpu(i), state.cpu_roles[i]));
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

const MAX_CLAIM_HEIGHT: i32 = 3;

fn get_knowledge(state: &State, participant: Participant) -> Option<&Knowledge> {
    match participant {
        Player => Some(&state.player_knowledge),
        Cpu(index) => state.cpu_knowledge.get(index),
    }
}
fn make_cpu_claim(state: &mut State, participant: Participant) {
    if participant == Player {
        return;
    }

    if let Some(role) = get_knowledge(state, participant).map(|k| k.role) {
        let claim = if is_werewolf(role) {
            //TODO better lying
            Claim { self_claim: Villager }

        } else {
            Claim { self_claim: role }
        };

        insert_claim(state, participant, claim);
    };
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
fn display_claim(platform: &Platform,
                 x: i32,
                 y: i32,
                 &(participant, claim): &(Participant, Claim)) {
    if participant == Player {
        (platform.print_xy)(x,
                            y,
                            &format!("You claim that you are a {}", claim.self_claim));
    } else {
        (platform.print_xy)(x,
                            y,
                            &format!("{} claims that they are a {}",
                                     participant,
                                     claim.self_claim));
    }
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
    //will be more complicated if we get to the expansions
    role == Werewolf
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

fn linear_search<T: PartialEq>(vector: &Vec<T>, thing: &T) -> Option<usize> {
    for i in 0..vector.len() {
        if thing == &vector[i] {
            return Some(i);
        }
    }

    None
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
    let size = (platform.size)();

    for i in 0..state.cpu_roles.len() {
        let index = i as i32;
        let cpu_player = Cpu(i);

        let spec = ButtonSpec {
            x: (size.width / 2) - 6,
            y: (index + 2) * 4,
            w: 11,
            h: 3,
            text: cpu_player.to_string(),
            id: 12 + index,
        };

        if do_button(platform,
                     &mut state.ui_context,
                     &spec,
                     left_mouse_pressed,
                     left_mouse_released) {
            return Some(cpu_player);
        }

    }

    None
}

fn get_vote(participant: Participant,
            participants: Vec<Participant>,
            knowledge: &Knowledge,
            rng: &mut StdRng)
            -> Participant {
    let filterd: Vec<Participant> = if is_werewolf(knowledge.role) {
        if let Some(&villager) = rng.choose(&knowledge.known_villagers) {
            return villager;
        }

        participants.iter()
            .filter(|p| **p != participant && !knowledge.known_werewolves.contains(p))
            .map(|&p| p)
            .collect()
    } else {
        if let Some(&werewolf) = rng.choose(&knowledge.known_werewolves) {
            return werewolf;
        }

        participants.iter()
            .filter(|p| **p != participant && !knowledge.known_villagers.contains(p))
            .map(|&p| p)
            .collect()
    };


    if let Some(&p) = rng.choose(&filterd) {
        return p;
    }

    //vote clockwise
    *(match participant {
              Player => participants.get(0),
              Cpu(i) => participants.get(i + 1),
          })
         .unwrap_or(&Player)
}

fn get_werewolves(state: &State) -> Vec<Participant> {
    let mut result = Vec::new();

    if state.player == Werewolf {
        result.push(Player);
    }

    for i in 0..state.cpu_roles.len() {
        if state.cpu_roles[i] == Werewolf {
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
