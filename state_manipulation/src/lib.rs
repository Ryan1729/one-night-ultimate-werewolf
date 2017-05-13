extern crate rand;
extern crate common;

use common::*;
use common::Role::*;
use common::Turn::*;
use common::Participant::*;

use rand::{StdRng, SeedableRng, Rng};

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

    let cpu_knowledge = cpu_roles.iter().map(|_| Knowledge::new()).collect();

    State {
        rng: rng,
        title_screen: title_screen,
        player,
        cpu_roles,
        table_roles,
        turn: Ready,
        player_knowledge: Knowledge::new(),
        cpu_knowledge,
        votes: Vec::new(),
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
        w: 11,
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

    match state.turn {
        Ready => {
            if ready_button(platform, state, left_mouse_pressed, left_mouse_released) {
                state.turn = state.turn.next();
            };
        }
        Werewolves => {
            let ready = if state.player == Werewolf {
                (platform.print_xy)(10,
                                    10,
                                    "Werewolves, wake up and look for other
werewolves.");

                ready_button(platform, state, left_mouse_pressed, left_mouse_released)

            } else {
                true
            };

            if ready {
                let werewolves = get_werewolves(state);

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
        // RobberTurn,
        // Discuss,
        Vote => {
            state.votes.clear();

            let player_vote = Cpu(0); //TODO

            state.votes.push(player_vote);

            for i in 0..state.cpu_knowledge.len() {
                let vote = get_vote(Cpu(i), &state.cpu_knowledge[i]);

                state.votes.push(vote);
            }

            state.turn = state.turn.next();
        }
        // Resolution,
        _ => {}
    }

    draw(platform, state);

    false
}

fn get_vote(p: Participant, knowledge: &Knowledge) -> Participant {
    //TODO decide based on knowledge and don't return p
    p
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
