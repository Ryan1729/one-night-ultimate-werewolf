extern crate rand;

use std::fmt;

use rand::StdRng;
use std::collections::HashMap;
use std::collections::HashSet;

pub struct Platform {
    pub print_xy: fn(i32, i32, &str),
    pub clear: fn(Option<Rect>),
    pub size: fn() -> Size,
    pub pick: fn(Point, i32) -> char,
    pub mouse_position: fn() -> Point,
    pub clicks: fn() -> i32,
    pub key_pressed: fn(KeyCode) -> bool,
    pub set_colors: fn(Color, Color),
    pub get_colors: fn() -> (Color, Color),
    pub set_foreground: fn(Color),
    pub get_foreground: fn() -> (Color),
    pub set_background: fn(Color),
    pub get_background: fn() -> (Color),
    pub set_layer: fn(i32),
    pub get_layer: fn() -> i32,
}

pub struct State {
    pub rng: StdRng,
    pub title_screen: bool,
    pub player: Role,
    pub initial_player: Role,
    pub cpu_roles: Vec<Role>,
    pub initial_cpu_roles: Vec<Role>,
    pub table_roles: [Role; 3],
    pub turn: Turn,
    pub player_knowledge: Knowledge,
    pub cpu_knowledge: Vec<Knowledge>,
    pub votes: Vec<(Participant, Participant)>,
    pub claims: HashMap<Participant, Claim>,
    pub ui_context: UIContext,
    pub role_spec: RoleSpec,
    pub show_role_spec: bool,
}

impl fmt::Debug for State {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("State")
            .field("title_screen", &self.title_screen)
            .field("player", &self.player)
            .field("cpu_roles", &self.cpu_roles)
            .field("table_roles", &self.table_roles)
            .field("turn", &self.turn)
            .field("player_knowledge", &self.player_knowledge)
            .field("cpu_knowledge", &self.cpu_knowledge)
            .field("votes", &self.votes)
            .field("ui_context", &self.ui_context)
            .finish()
    }
}

#[derive(Clone,Copy, Debug, PartialEq, Eq,PartialOrd, Ord, Hash)]
pub enum Role {
    //TODO before adding expansions, it should be made much easier to add a role.
    Werewolf,
    Minion,
    Robber,
    Mason,
    Seer,
    Troublemaker,
    Drunk,
    Insomniac,
    Villager,
    Tanner,
    Hunter,
    DoppelWerewolf(Participant),
    DoppelMinion(Participant),
    DoppelRobber(Participant),
    DoppelMason(Participant),
    DoppelSeer(Participant),
    DoppelTroublemaker(Participant),
    DoppelDrunk(Participant),
    DoppelInsomniac(Participant),
    DoppelVillager(Participant),
    DoppelTanner(Participant),
    DoppelHunter(Participant),
}
use Role::*;

//2 werewolves, 3 villagers, 2 masons and 1 everytthing else
const ROLE_CARDS_AVAILABLE: u8 = 2 + 3 + 2 + 9;

pub fn get_doppel_role(role: Role, participant: Participant) -> Role {
    match role {
        Werewolf => DoppelWerewolf(participant),
        Minion => DoppelMinion(participant),
        Robber => DoppelRobber(participant),
        Mason => DoppelMason(participant),
        Seer => DoppelSeer(participant),
        Troublemaker => DoppelTroublemaker(participant),
        Drunk => DoppelDrunk(participant),
        Insomniac => DoppelInsomniac(participant),
        Villager => DoppelVillager(participant),
        Tanner => DoppelTanner(participant),
        Hunter => DoppelHunter(participant),
        DoppelWerewolf(_) => DoppelWerewolf(participant),
        DoppelMinion(_) => DoppelMinion(participant),
        DoppelRobber(_) => DoppelRobber(participant),
        DoppelMason(_) => DoppelMason(participant),
        DoppelSeer(_) => DoppelSeer(participant),
        DoppelTroublemaker(_) => DoppelTroublemaker(participant),
        DoppelDrunk(_) => DoppelDrunk(participant),
        DoppelInsomniac(_) => DoppelInsomniac(participant),
        DoppelVillager(_) => DoppelVillager(participant),
        DoppelTanner(_) => DoppelTanner(participant),
        DoppelHunter(_) => DoppelHunter(participant),
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:b}{:o}", *self, *self)
    }
}

//here we're abusigng the Octal trait since (currently) we can't make a custom display attribute
//o for "only the name"?
impl fmt::Octal for Role {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
               "{}",
               match *self {
                   Werewolf => "Werewolf",
                   Minion => "Minion",
                   Mason => "Mason",
                   Robber => "Robber",
                   Seer => "Seer",
                   Troublemaker => "Troublemaker",
                   Drunk => "Drunk",
                   Insomniac => "Insomniac",
                   Villager => "Villager",
                   Tanner => "Tanner",
                   Hunter => "Hunter",
                   //We'll assume don't know what the doppelganger copied in the general case
                   DoppelWerewolf(_) |
                   DoppelMinion(_) |
                   DoppelMason(_) |
                   DoppelRobber(_) |
                   DoppelSeer(_) |
                   DoppelTroublemaker(_) |
                   DoppelDrunk(_) |
                   DoppelInsomniac(_) |
                   DoppelVillager(_) |
                   DoppelTanner(_) |
                   DoppelHunter(_) => "Doppelganger",
               })
    }
}

//and the formatting trait abuse continues!
//b fo "before the name"
impl fmt::Binary for Role {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
               "{}",
               match *self {
                   Insomniac => "an ",
                   Werewolf |
                   Minion |
                   Mason |
                   Robber |
                   Seer |
                   Troublemaker |
                   Drunk |
                   Villager |
                   Tanner |
                   Hunter |
                   DoppelWerewolf(_) |
                   DoppelMinion(_) |
                   DoppelMason(_) |
                   DoppelRobber(_) |
                   DoppelSeer(_) |
                   DoppelTroublemaker(_) |
                   DoppelDrunk(_) |
                   DoppelInsomniac(_) |
                   DoppelVillager(_) |
                   DoppelTanner(_) |
                   DoppelHunter(_) => "a ",
               })
    }
}

pub fn full_role_string(role: Role) -> String {
    let role_name = match role {
        DoppelWerewolf(_) => "Doppel-Werewolf".to_string(),
        DoppelMinion(_) => "Doppel-Minion".to_string(),
        DoppelMason(_) => "Doppel-Mason".to_string(),
        DoppelRobber(_) => "Doppel-Robber".to_string(),
        DoppelSeer(_) => "Doppel-Seer".to_string(),
        DoppelTroublemaker(_) => "Doppel-Troublemaker".to_string(),
        DoppelDrunk(_) => "Doppel-Drunk".to_string(),
        DoppelInsomniac(_) => "Doppel-Insomniac".to_string(),
        DoppelVillager(_) => "Doppel-Villager".to_string(),
        DoppelTanner(_) => "Doppel-Tanner".to_string(),
        DoppelHunter(_) => "Doppel-Hunter".to_string(),

        _ => format!("{:o}", role),
    };

    format!("{:b}{}", role, role_name)
}

#[derive(Clone,Copy, PartialEq, Debug)]
pub struct RoleSpec {
    pub villager1: bool,
    pub villager2: bool,
    pub villager3: bool,
    pub werewolf1: bool,
    pub werewolf2: bool,
    pub seer: bool,
    pub robber: bool,
    pub troublemaker: bool,
    pub tanner: bool,
    pub drunk: bool,
    pub hunter: bool,
    pub masons: bool,
    pub insomniac: bool,
    pub minion: bool,
    pub doppelganger: bool,
}

macro_rules! add_role{
    ($result:expr, $flag:expr, $role:expr) => {
        if $flag {
            $result.push($role);
        }
    }
}

macro_rules! bool_to_int {
    ($($int: expr), +) =>  {
        0 $( + if $int {
            1
        } else {
            0
        })+
    }
}


//2 cpu players minimum, 1 for the player and 3 for the center
const MINIMUM_CARDS : usize = 2 + 1 + 3;

impl RoleSpec {
    pub fn get_role_vector(&self) -> Vec<Role> {

        let mut result = Vec::new();

        add_role!(result, self.villager1, Villager);
        add_role!(result, self.villager2, Villager);
        add_role!(result, self.villager3, Villager);
        add_role!(result, self.werewolf1, Werewolf);
        add_role!(result, self.werewolf2, Werewolf);
        add_role!(result, self.seer, Seer);
        add_role!(result, self.robber, Robber);
        add_role!(result, self.troublemaker, Troublemaker);
        add_role!(result, self.tanner, Tanner);
        add_role!(result, self.drunk, Drunk);
        add_role!(result, self.hunter, Hunter);
        //only either 0 or 2 masons
        add_role!(result, self.masons, Mason);
        add_role!(result, self.masons, Mason);
        add_role!(result, self.insomniac, Insomniac);
        add_role!(result, self.minion, Minion);
        //DoppelVillager(Player) represents the Doppelganger card
        add_role!(result, self.doppelganger, DoppelVillager(Player));

        while result.len() < MINIMUM_CARDS {
            result.insert(0, Villager);
        }

        result
    }

    pub fn get_cpu_player_count(&self, optional_role_vector:Option<&Vec<Role>>) -> u32 {
        if let Some(v) = optional_role_vector{
            RoleSpec::get_cpu_player_count_from_vector(&v)
        } else {
            RoleSpec::get_cpu_player_count_from_vector(&self.get_role_vector())
        }
    }

    fn get_cpu_player_count_from_vector(role_vector: &Vec<Role>) -> u32 {
        (role_vector.len() as u32).saturating_sub(3 + 1)
    }

    pub fn get_count(&self, role: &Role) -> u32 {
        match *role {
            Werewolf => bool_to_int!(self.werewolf1, self.werewolf2),
            Minion => bool_to_int!(self.minion),
            Robber => bool_to_int!(self.robber),
            //only either 0 or 2 masons
            Mason => bool_to_int!(self.masons) * 2,
            Seer => bool_to_int!(self.seer),
            Troublemaker => bool_to_int!(self.troublemaker),
            Drunk => bool_to_int!(self.drunk),
            Insomniac => bool_to_int!(self.insomniac),
            Villager => bool_to_int!(self.villager1, self.villager2, self.villager3),
            Tanner => bool_to_int!(self.tanner),
            Hunter => bool_to_int!(self.hunter),
            DoppelWerewolf(_) |
            DoppelMinion(_) |
            DoppelRobber(_) |
            DoppelMason(_) |
            DoppelSeer(_) |
            DoppelTroublemaker(_) |
            DoppelDrunk(_) |
            DoppelInsomniac(_) |
            DoppelVillager(_) |
            DoppelTanner(_) |
            DoppelHunter(_) => bool_to_int!(self.doppelganger),
        }
    }

    pub fn add(&mut self, role: &Role) {
        match *role {
            Werewolf => {
                if self.werewolf1 {
                    self.werewolf2 = true;
                } else {
                    self.werewolf1 = true;
                }
            }
            Minion => {
                self.minion = true;
            }
            Robber => {
                self.robber = true;
            }
            //only either 0 or 2 masons
            Mason => {
                self.masons = true;
            }
            Seer => {
                self.seer = true;
            }
            Troublemaker => {
                self.troublemaker = true;
            }
            Drunk => {
                self.drunk = true;
            }
            Insomniac => {
                self.insomniac = true;
            }
            Villager => {
                if self.villager1 {
                    if self.villager2 {
                        self.villager3 = true;
                    } else {
                        self.villager2 = true;
                    }
                } else {
                    self.villager1 = true;
                }
            }
            Tanner => {
                self.tanner = true;
            }
            Hunter => {
                self.hunter = true;
            }
            DoppelWerewolf(_) |
            DoppelMinion(_) |
            DoppelRobber(_) |
            DoppelMason(_) |
            DoppelSeer(_) |
            DoppelTroublemaker(_) |
            DoppelDrunk(_) |
            DoppelInsomniac(_) |
            DoppelVillager(_) |
            DoppelTanner(_) |
            DoppelHunter(_) => {
                self.doppelganger = true;
            }
        }
    }

    pub fn can_add(&mut self, role: &Role) -> bool {
        let cpu_player_count = self.get_cpu_player_count(None);

        if cpu_player_count >= 9 {
            return false;
        }

        match *role {
            Werewolf => {
                !self.werewolf2 || !self.werewolf1
            }
            Minion => {
                !self.minion
            }
            Robber => {
                !self.robber
            }
            Mason =>  !self.masons && cpu_player_count < 8,
            Seer => {
                !self.seer
            }
            Troublemaker => {
                !self.troublemaker
            }
            Drunk => {
                !self.drunk
            }
            Insomniac => {
                !self.insomniac
            }
            Villager => {
                !self.villager3 || !self.villager2 || !self.villager1
            }
            Tanner => {
                !self.tanner
            }
            Hunter => {
                !self.hunter
            }
            DoppelWerewolf(_) |
            DoppelMinion(_) |
            DoppelRobber(_) |
            DoppelMason(_) |
            DoppelSeer(_) |
            DoppelTroublemaker(_) |
            DoppelDrunk(_) |
            DoppelInsomniac(_) |
            DoppelVillager(_) |
            DoppelTanner(_) |
            DoppelHunter(_) => {
                !self.doppelganger
            }
            }
    }

    pub fn remove(&mut self, role: &Role) {
        if self.can_remove(role) {
        match *role {
            Werewolf => {
                if self.werewolf2 {
                    self.werewolf2 = false;
                } else {
                    self.werewolf1 = false;
                }
            }
            Minion => {
                self.minion = false;
            }
            Robber => {
                self.robber = false;
            }
            Mason =>  {self.masons = false;}
            Seer => {
                self.seer = false;
            }
            Troublemaker => {
                self.troublemaker = false;
            }
            Drunk => {
                self.drunk = false;
            }
            Insomniac => {
                self.insomniac = false;
            }
            Villager => {
                if self.villager3 {
                    self.villager3 = false;
                } else {
                    if self.villager2 {
                        self.villager2 = false;
                    } else {
                        self.villager1 = false;
                    }
                }
            }
            Tanner => {
                self.tanner = false;
            }
            Hunter => {
                self.hunter = false;
            }
            DoppelWerewolf(_) |
            DoppelMinion(_) |
            DoppelRobber(_) |
            DoppelMason(_) |
            DoppelSeer(_) |
            DoppelTroublemaker(_) |
            DoppelDrunk(_) |
            DoppelInsomniac(_) |
            DoppelVillager(_) |
            DoppelTanner(_) |
            DoppelHunter(_) => {
                self.doppelganger = false;
            }
        }
    };
    }

    pub fn can_remove(&mut self, role: &Role) -> bool {
        let cpu_player_count = self.get_cpu_player_count(None);

        if cpu_player_count < 3 {
            return false;
        }

        match *role {
            Werewolf => {
                 self.werewolf2 || self.werewolf1
            }
            Minion => {
                self.minion
            }
            Robber => {
                self.robber
            }
            Mason =>  self.masons && cpu_player_count >= 4,
            Seer => {
                self.seer
            }
            Troublemaker => {
                self.troublemaker
            }
            Drunk => {
                self.drunk
            }
            Insomniac => {
                self.insomniac
            }
            Villager => {
                self.villager3 || self.villager2 ||self.villager1
            }
            Tanner => {
                self.tanner
            }
            Hunter => {
                self.hunter
            }
            DoppelWerewolf(_) |
            DoppelMinion(_) |
            DoppelRobber(_) |
            DoppelMason(_) |
            DoppelSeer(_) |
            DoppelTroublemaker(_) |
            DoppelDrunk(_) |
            DoppelInsomniac(_) |
            DoppelVillager(_) |
            DoppelTanner(_) |
            DoppelHunter(_) => {
                self.doppelganger
            }
}
    }
}

impl Default for RoleSpec {
    fn default() -> RoleSpec {
        RoleSpec {
            werewolf1: true,
            werewolf2: true,
            villager1: true,
            villager2: true,
            villager3: false,
            seer: true,
            robber: true,
            troublemaker: true,
            tanner: false,
            drunk: false,
            hunter: false,
            masons: false,
            insomniac: false,
            minion: false,
            doppelganger: false,
        }
    }
}

impl Rand for RoleSpec {
    fn rand<R: Rng>(rng: &mut R) -> Self {
        //TODO Insomniac is pointless if there isn't at last one role that moves card around
        let cpu_player_count = rng.gen_range(2, 10);
        // 3 in the center and 1 for the player
        let mut roles_needed = 3 + 1 + cpu_player_count;
        let mut deck_size = ROLE_CARDS_AVAILABLE;

        let masons = roles_needed > ROLE_CARDS_AVAILABLE - 2 || rng.gen::<bool>();

        if masons {
            roles_needed -= 2;
        }
        deck_size -= 2;

        //always at least on werewolf
        let werewolf1 = true;
        roles_needed -= 1;
        deck_size -= 1;

        let difference = deck_size - roles_needed;

        let deck_iter = std::iter::repeat(true)
            .take(roles_needed as usize)
            .chain(std::iter::repeat(false).take(difference as usize));

        let mut deck: Vec<bool> = deck_iter.collect();

        rng.shuffle(&mut deck);

        RoleSpec {
            werewolf1,
            werewolf2: deck.pop().unwrap_or(false),
            seer: deck.pop().unwrap_or(false),
            robber: deck.pop().unwrap_or(false),
            troublemaker: deck.pop().unwrap_or(false),
            tanner: deck.pop().unwrap_or(false),
            drunk: deck.pop().unwrap_or(false),
            hunter: deck.pop().unwrap_or(false),
            masons,
            insomniac: deck.pop().unwrap_or(false),
            minion: deck.pop().unwrap_or(false),
            doppelganger: deck.pop().unwrap_or(false),
            //not 100% sure but I think handling masons causes the deck to
            //to run out sometimes. If so, it should run out on villagers.
            villager1: deck.pop().unwrap_or(false),
            villager2: deck.pop().unwrap_or(false),
            villager3: deck.pop().unwrap_or(false),
        }
    }
}

#[derive(Clone,Copy, PartialEq, Debug)]
pub enum Turn {
    Ready,
    SeeRole(bool),
    DoppelSeerTurn,
    DoppelSeerRevealOne(Participant),
    DoppelSeerRevealTwo(CenterPair),
    DoppelRobberTurn,
    DoppelRobberReveal,
    DoppelTroublemakerTurn,
    DoppelTroublemakerSecondChoice(Participant),
    DoppelDrunkTurn,
    DoppelMinionTurn,
    Werewolves,
    MinionTurn,
    MasonTurn,
    SeerTurn,
    SeerRevealOne(Participant),
    SeerRevealTwo(CenterPair),
    RobberTurn,
    RobberReveal,
    TroublemakerTurn,
    TroublemakerSecondChoice(Participant),
    DrunkTurn,
    InsomniacTurn,
    DoppelInsomniacTurn,
    BeginDiscussion,
    Discuss,
    Vote,
    Resolution,
}
use Turn::*;

impl Turn {
    pub fn next(&self) -> Turn {
        match *self {
            //we only need the (*)Reveal states when the player is the (*)
            Ready => SeeRole(false),
            SeeRole(_) => DoppelSeerTurn,
            DoppelSeerTurn => DoppelRobberTurn,
            DoppelSeerRevealOne(_) => DoppelSeerTurn.next(),
            DoppelSeerRevealTwo(_) => DoppelSeerTurn.next(),
            DoppelRobberTurn => DoppelTroublemakerTurn,
            DoppelRobberReveal => DoppelRobberTurn.next(),
            DoppelTroublemakerSecondChoice(_) => DoppelTroublemakerTurn.next(),
            DoppelTroublemakerTurn => DoppelDrunkTurn,
            DoppelDrunkTurn => DoppelMinionTurn,
            DoppelMinionTurn => Werewolves,
            Werewolves => MinionTurn,
            MinionTurn => MasonTurn,
            MasonTurn => SeerTurn,
            SeerTurn => RobberTurn,
            SeerRevealOne(_) => SeerTurn.next(),
            SeerRevealTwo(_) => SeerTurn.next(),
            RobberTurn => TroublemakerTurn,
            RobberReveal => RobberTurn.next(),
            TroublemakerSecondChoice(_) => TroublemakerTurn.next(),
            TroublemakerTurn => DrunkTurn,
            DrunkTurn => InsomniacTurn,
            InsomniacTurn => DoppelInsomniacTurn,
            DoppelInsomniacTurn => BeginDiscussion,
            BeginDiscussion => Discuss,
            Discuss => Vote,
            Vote => Resolution,
            Resolution => Ready,
        }
    }
}

pub trait AllValues {
    fn all_values() -> Vec<Self> where Self: std::marker::Sized;
}

use rand::Rand;
use rand::Rng;

macro_rules! all_values_rand_impl {
    ($($t:ty)*) => ($(
        impl Rand for $t {
            fn rand<R: Rng>(rng: &mut R) -> Self {
                let values = Self::all_values();

                let len = values.len();

                if len == 0 {
                    panic!("Cannot pick a random value because T::all_values()\
 returned an empty vector!")
                } else {
                    let i = rng.gen_range(0, len);

                    values[i]
                }
            }
        }
    )*)
}

#[derive(PartialEq, Eq,PartialOrd, Ord, Clone,Copy, Debug)]
pub enum CenterPair {
    FirstSecond,
    FirstThird,
    SecondThird,
}
use CenterPair::*;

impl AllValues for CenterPair {
    fn all_values() -> Vec<CenterPair> {
        vec![FirstSecond, FirstThird, SecondThird]
    }
}

all_values_rand_impl!(CenterPair);

impl fmt::Display for CenterPair {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {

        write!(f,
               "{}",
               match *self {
                   FirstSecond => "First and Second",
                   FirstThird => "First and Third",
                   SecondThird => "Second and Third",
               })
    }
}

#[derive(PartialEq, Eq,PartialOrd, Ord, Clone,Copy, Debug)]
pub enum CenterCard {
    First,
    Second,
    Third,
}
use CenterCard::*;

impl AllValues for CenterCard {
    fn all_values() -> Vec<CenterCard> {
        vec![First, Second, Third]
    }
}

all_values_rand_impl!(CenterCard);

impl fmt::Display for CenterCard {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {

        write!(f,
               "{}",
               match *self {
                   First => "First",
                   Second => "Second",
                   Third => "Third",
               })
    }
}

#[derive(Clone,Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Participant {
    Player,
    Cpu(usize),
}
use Participant::*;

impl fmt::Display for Participant {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {

        write!(f,
               "{}",
               match *self {
                   Player => "You".to_string(),
                   Cpu(i) => format!("Cpu {}", i),
               })
    }
}

#[derive(Debug, Clone)]
pub struct Knowledge {
    pub known_werewolves: HashSet<Participant>,
    pub known_villagers: HashSet<Participant>,
    pub role: Role,
    pub true_claim: Claim,
    //TODO is this needed/used?
    //Is it filled maximumlly?
    pub known_non_active: HashSet<Role>,
    //TODO should this line be here?
    //pub known_doppel: Option<Participant>,
    //TODO should these account for the possibity of a /Doppel(Minion|Tanner)/ ?
    pub known_minion: Option<Participant>,
    pub known_tanner: Option<Participant>,
    pub robber_swap: Option<(Participant, Participant, Role)>,
    pub troublemaker_swap: Option<(Participant, Participant)>,
    pub drunk_swap: Option<(Participant, CenterCard)>,
    pub insomniac_peek: bool,
}

impl Knowledge {
    pub fn new(role: Role, participant: Participant) -> Self {

        let (known_minion, known_tanner) = match role {
            Minion => (Some(participant), None),
            Tanner => (None, Some(participant)),
            _ => (None, None),
        };

        let true_claim = match role {
            DoppelVillager(p) => DoppelSimple(p, Villager),
            DoppelTanner(p) => DoppelSimple(p, Tanner),
            DoppelHunter(p) => DoppelSimple(p, Hunter),
            _ => Simple(role),
        };

        Knowledge {
            known_werewolves: HashSet::new(),
            known_villagers: HashSet::new(),
            role,
            true_claim,
            known_non_active: HashSet::new(),
            known_minion,
            known_tanner,
            robber_swap: None,
            troublemaker_swap: None,
            drunk_swap: None,
            insomniac_peek: false,
        }
    }
}

#[derive(PartialEq, Eq,PartialOrd, Ord, Clone,Copy, Debug)]
pub enum Claim {
    Simple(Role),
    DoppelSimple(Participant, Role),
    MasonAction(ZeroToTwo<Participant>),
    DoppelMasonAction(Participant, ZeroToTwo<Participant>),
    RobberAction(Participant, Role),
    DoppelRobberAction(Participant, Participant, Role),
    SeerRevealOneAction(Participant, Role),
    DoppelSeerRevealOneAction(Participant, Participant, Role),
    SeerRevealTwoAction(CenterPair, Role, Role),
    DoppelSeerRevealTwoAction(Participant, CenterPair, Role, Role),
    TroublemakerAction(Participant, Participant),
    DoppelTroublemakerAction(Participant, Participant, Participant),
    InsomniacAction(Role),
    DoppelInsomniacAction(Participant, Role),
    DrunkAction(CenterCard),
    DoppelDrunkAction(Participant, CenterCard),
}
use Claim::*;

#[derive(PartialEq, Eq,PartialOrd, Ord, Clone,Copy, Debug)]
pub enum ZeroToTwo<T> {
    Zero,
    One(T),
    Two(T, T),
}

pub type UiId = i32;

#[derive(Debug)]
pub struct UIContext {
    pub hot: UiId,
    pub active: UiId,
    pub next_hot: UiId,
}

impl UIContext {
    pub fn new() -> Self {
        UIContext {
            hot: 0,
            active: 0,
            next_hot: 0,
        }
    }

    pub fn set_not_active(&mut self) {
        self.active = 0;
    }
    pub fn set_active(&mut self, id: UiId) {
        self.active = id;
    }
    pub fn set_next_hot(&mut self, id: UiId) {
        self.next_hot = id;
    }
    pub fn set_not_hot(&mut self) {
        self.hot = 0;
    }
    pub fn frame_init(&mut self) {
        if self.active == 0 {
            self.hot = self.next_hot;
        }
        self.next_hot = 0;
    }
}

pub enum Direction {
    Right,
    Left,
}









//NOTE(Ryan1729): if I import BearLibTerminal.rs into `state_manipulation` or a crate
//`state_manipulation` depends on, like this one for example, then the
//ffi to the C version of BearLibTerminal causes an error. I just want
//the geometry datatypes and the Event and Keycode definitions so I have
//copied them from BearLibTerminal.rs below

//BearLibTerminal.rs is released under the MIT license by nabijaczleweli.
//see https://github.com/nabijaczleweli/BearLibTerminal.rs/blob/master/LICENSE
//for full details.

impl Point {
    /// Creates a new point on the specified non-negative coordinates
    pub fn new_safe(mut x: i32, mut y: i32) -> Point {
        x = if x >= 0 { x } else { 0 };
        y = if y >= 0 { y } else { 0 };

        Point { x: x, y: y }
    }

    pub fn add(&self, x: i32, y: i32) -> Point {
        Point::new_safe(self.x + x, self.y + y)
    }
}

/// Represents a single on-screen point/coordinate pair.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    /// Creates a new point on the specified non-negative coordinates
    pub fn new(x: i32, y: i32) -> Point {
        assert!(x >= 0);
        assert!(y >= 0);

        Point { x: x, y: y }
    }
}


/// A 2D size representation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

impl Size {
    /// Creates a new non-negative size.
    pub fn new(width: i32, height: i32) -> Size {
        assert!(width >= 0);
        assert!(height >= 0);

        Size {
            width: width,
            height: height,
        }
    }
}

impl fmt::Display for Size {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}x{}", self.width, self.height)
    }
}

/// A rectangle, described by its four corners and a size.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Rect {
    /// The top-left corner.
    pub top_left: Point,
    /// The top-right corner.
    pub top_right: Point,
    /// The bottom-right corner.
    pub bottom_right: Point,
    /// The bottom-left corner.
    pub bottom_left: Point,
    /// The `Rect`angle's size.
    pub size: Size,
}

impl Rect {
    /// Construct a `Rect` from its top-left corner and its size.
    ///
    /// # Examples
    ///
    /// ```
    /// # use bear_lib_terminal::geometry::{Rect, Point, Size};
    /// let rect = Rect::from_size(Point::new(10, 20), Size::new(30, 40));
    /// assert_eq!(rect.top_left, Point::new(10, 20));
    /// assert_eq!(rect.top_right, Point::new(40, 20));
    /// assert_eq!(rect.bottom_left, Point::new(10, 60));
    /// assert_eq!(rect.bottom_right, Point::new(40, 60));
    /// assert_eq!(rect.size, Size::new(30, 40));
    /// ```
    pub fn from_size(origin: Point, size: Size) -> Rect {
        let top_right = Point::new(origin.x + size.width, origin.y);
        let bottom_left = Point::new(origin.x, origin.y + size.height);
        let bottom_right = Point::new(top_right.x, bottom_left.y);

        Rect {
            top_left: origin,
            top_right: top_right,
            bottom_left: bottom_left,
            bottom_right: bottom_right,
            size: size,
        }
    }

    /// Construct a `Rect` from its top-left and bottom-right corners.
    ///
    /// # Examples
    ///
    /// ```
    /// # use bear_lib_terminal::geometry::{Rect, Point, Size};
    /// let rect = Rect::from_points(Point::new(10, 20), Point::new(30, 40));
    /// assert_eq!(rect.top_left, Point::new(10, 20));
    /// assert_eq!(rect.top_right, Point::new(30, 20));
    /// assert_eq!(rect.bottom_left, Point::new(10, 40));
    /// assert_eq!(rect.bottom_right, Point::new(30, 40));
    /// assert_eq!(rect.size, Size::new(20, 20));
    /// ```
    pub fn from_points(top_left: Point, bottom_right: Point) -> Rect {
        assert!(bottom_right.x >= top_left.x);
        assert!(bottom_right.y >= top_left.y);

        let size = Size::new(bottom_right.x - top_left.x, bottom_right.y - top_left.y);
        Rect::from_size(top_left, size)
    }

    /// Construct a `Rect` from its top-left corner and its size, values unwrapped.
    ///
    /// # Examples
    ///
    /// ```
    /// # use bear_lib_terminal::geometry::{Rect, Point, Size};
    /// assert_eq!(Rect::from_values(10, 20, 30, 40),
    ///     Rect::from_size(Point::new(10, 20), Size::new(30, 40)));
    /// ```
    pub fn from_values(x: i32, y: i32, width: i32, height: i32) -> Rect {
        let origin = Point::new(x, y);
        let size = Size::new(width, height);
        Rect::from_size(origin, size)
    }


    /// Construct a `Rect` from its top-left and bottom-right corners, values unwrapped.
    ///
    /// # Examples
    ///
    /// ```
    /// # use bear_lib_terminal::geometry::{Rect, Point, Size};
    /// assert_eq!(Rect::from_point_values(10, 20, 30, 40),
    ///     Rect::from_points(Point::new(10, 20), Point::new(30, 40)));
    /// ```
    pub fn from_point_values(top_left_x: i32,
                             top_left_y: i32,
                             bottom_right_x: i32,
                             bottom_right_y: i32)
                             -> Rect {
        let top_left = Point::new(top_left_x, top_left_y);
        let bottom_right = Point::new(bottom_right_x, bottom_right_y);
        Rect::from_points(top_left, bottom_right)
    }
}

//input module

/// All pressable keys.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum KeyCode {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    /// Top-row `1/!` key.
    Row1,
    /// Top-row `2/@` key.
    Row2,
    /// Top-row `3/#` key.
    Row3,
    /// Top-row `4/$` key.
    Row4,
    /// Top-row `5/%` key.
    Row5,
    /// Top-row `6/^` key.
    Row6,
    /// Top-row `7/&` key.
    Row7,
    /// Top-row `8/*` key.
    Row8,
    /// Top-row `9/(` key.
    Row9,
    /// Top-row `0/)` key.
    Row0,
    /// Top-row &#96;/~ key.
    Grave,
    /// Top-row `-/_` key.
    Minus,
    /// Top-row `=/+` key.
    Equals,
    /// Second-row `[/{` key.
    LeftBracket,
    /// Second-row `]/}` key.
    RightBracket,
    /// Second-row `\/|` key.
    Backslash,
    /// Third-row `;/:` key.
    Semicolon,
    /// Third-row `'/"` key.
    Apostrophe,
    /// Fourth-row `,/<` key.
    Comma,
    /// Fourth-row `./>` key.
    Period,
    /// Fourth-row `//?` key.
    Slash,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Enter,
    Escape,
    Backspace,
    Tab,
    Space,
    Pause,
    Insert,
    Home,
    PageUp,
    Delete,
    End,
    PageDown,
    /// Right arrow key.
    Right,
    /// Left arrow key.
    Left,
    /// Down arrow key.
    Down,
    /// Up arrow key.
    Up,
    /// Numpad `/` key.
    NumDivide,
    /// Numpad `*` key.
    NumMultiply,
    /// Numpad `-` key.
    NumMinus,
    /// Numpad `+` key.
    NumPlus,
    /// Numpad &#9166; key.
    NumEnter,
    /// Numpad `Del/.` key (output locale-dependent).
    NumPeriod,
    /// Numpad `1/End` key.
    Num1,
    /// Numpad 2/&#8595; key.
    Num2,
    /// Numpad `3/PageDown` key.
    Num3,
    /// Numpad 4/&#8592; key.
    Num4,
    /// Numpad `5` key.
    Num5,
    /// Numpad 6/&#8594; key.
    Num6,
    /// Numpad `7/Home` key.
    Num7,
    /// Numpad 8/&#8593; key.
    Num8,
    /// Numpad `9/PageUp` key.
    Num9,
    /// Numpad `0/Insert` key.
    Num0,
    /// Left mouse button.
    MouseLeft,
    /// Right mouse button.
    MouseRight,
    /// Middle mouse button a.k.a. pressed scroll wheel.
    MouseMiddle,
    MouseFourth,
    MouseFifth,
}

/// A single input event.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Event {
    /// Terminal window closed.
    Close,
    /// Terminal window resized. Needs to have `window.resizeable = true` to occur.
    ///
    /// Note, that the terminal window is cleared when resized.
    Resize {
        /// Width the terminal was resized to.
        width: i32,
        /// Heigth the terminal was resized to.
        height: i32,
    },
    /// Mouse moved.
    ///
    /// If [`precise-mouse`](config/struct.Input.html#structfield.precise_mouse) is off,
    /// generated each time mouse moves from cell to cell, otherwise,
    /// when it moves from pixel to pixel.
    MouseMove {
        /// `0`-based cell index from the left to which the mouse cursor moved.
        x: i32,
        /// `0`-based cell index from the top to which the mouse cursor moved.
        y: i32,
    },
    /// Mouse wheel moved.
    MouseScroll {
        /// Amount of steps the wheel rotated.
        ///
        /// Positive when scrolled "down"/"backwards".
        ///
        /// Negative when scrolled "up"/"forwards"/"away".
        delta: i32,
    },
    /// A keyboard or mouse button pressed (might repeat, if set in OS).
    KeyPressed {
        /// The key pressed.
        key: KeyCode,
        /// Whether the Control key is pressed.
        ctrl: bool,
        /// Whether the Shift key is pressed.
        shift: bool,
    },
    /// A keyboard or mouse button released.
    KeyReleased {
        /// The key released.
        key: KeyCode,
        /// Whether the Control key is pressed.
        ctrl: bool,
        /// Whether the Shift key is pressed.
        shift: bool,
    },
    /// The Shift key pressed (might repeat, if set in OS).
    ShiftPressed,
    /// The Shift key released.
    ShiftReleased,
    /// The Shift key pressed (might repeat, if set in OS).
    ControlPressed,
    /// The Control key released.
    ControlReleased,
}

pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}
