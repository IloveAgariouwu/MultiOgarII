use super::cell::{size_from_mass, Kind};
use super::player::{ROLE_ADMIN, ROLE_GUEST, ROLE_MODER};
use super::World;
use crate::core::logger;
use crate::net::outbox::CLOSE_NO_STATUS;

struct ChatCommand {
    name: &'static str,
    description: &'static str,
    usage: &'static str,
    min_role: u8,
}

const fn cmd(
    name: &'static str,
    description: &'static str,
    usage: &'static str,
    min_role: u8,
) -> ChatCommand {
    ChatCommand { name, description, usage, min_role }
}

const CHAT_COMMANDS: &[ChatCommand] = &[
    cmd("help", "lists available commands, or command usage", "[cmd]", ROLE_GUEST),
    cmd("id", "gets your playerID", "", ROLE_GUEST),
    cmd("kill", "self kill", "", ROLE_GUEST),
    cmd("skin", "change skin", "[skin]", ROLE_GUEST),
    cmd("login", "upgrade your credential to access more commands", "<password>", ROLE_GUEST),
    cmd("logout", "remove your credentials", "", ROLE_GUEST),
    cmd("mass", "gives mass to yourself or to other player", "<mass> [id]", ROLE_MODER),
    cmd("status", "shows status of the server", "", ROLE_MODER),
    cmd("add-minion", "gives yourself or other player minions", "<count> [id] [name]", ROLE_MODER),
    cmd("rm-minion", "removes minions from yourself or others", "[id]", ROLE_MODER),
    cmd("killall", "kills everyone", "", ROLE_MODER),
    cmd("spawnmass", "gives yourself or other player spawnmass", "<mass> [id]", ROLE_ADMIN),
    cmd("addbot", "adds AI bots to the server", "<count>", ROLE_ADMIN),
    cmd("shutdown", "SHUTS DOWN THE SERVER", "", ROLE_ADMIN),
    cmd("restart", "restarts the server", "", ROLE_ADMIN),
];

const MAX_ADDED_AT_ONCE: i64 = 1000;

const CONSOLE_HELP: &[(&str, &str)] = &[
    ("help", "List of currently available commands."),
    ("player", "List the stats of a particular player or get a list of players and their respective IDs."),
    ("minion", "Give minions to a player."),
    ("addbot", "Add player bots to this server."),
    ("rmbot", "Remove bots from the server"),
    ("kick", "Kick a client from the game"),
    ("killall", "Remove all client cells from the game."),
    ("mass", "Set the mass of a specific client."),
    ("exit", "Exit the server."),
    ("stats", "Generate current server stats"),
    ("aliases", "Generate command aliases."),
];

fn alias_of(command: &str) -> String {
    let first = &command[..1];
    let last = &command[command.len() - 1..];
    format!("{first}{last}")
}

pub fn parse_int(text: &str) -> Option<i64> {
    let text = text.trim_start();
    let (sign, rest) = match text.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, text.strip_prefix('+').unwrap_or(text)),
    };
    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    rest[..digits].parse::<i64>().ok().map(|value| value * sign)
}

fn int(arg: Option<&&str>) -> Option<i64> {
    arg.and_then(|text| parse_int(text))
}

pub fn parse_mass(arg: Option<&&str>) -> Option<f64> {
    int(arg).filter(|mass| *mass >= 0).map(|mass| mass as f64)
}

impl World {
    fn command_target(&self, from: u32, arg: Option<&&str>, warning: &str) -> Option<u32> {
        match int(arg) {
            None => {
                if !warning.is_empty() {
                    self.tell(from, warning);
                }
                Some(from)
            }
            Some(pid) => {
                let found = u32::try_from(pid).ok().and_then(|pid| self.player_by_pid(pid));
                if found.is_none() {
                    self.tell(from, &format!("Didn't find player with id {pid}"));
                }
                found
            }
        }
    }

    pub fn player_command(&mut self, from: u32, line: &str) {
        let args: Vec<&str> = line.split(' ').collect();
        let Some(command) = CHAT_COMMANDS.iter().find(|c| c.name == args[0]) else {
            self.tell(from, "Invalid command, please use /help to get a list of available commands");
            return;
        };
        let role = self.player(from).role;
        if role < command.min_role {
            self.tell(from, &format!("ERROR: access denied! {role}, {}", command.min_role));
            return;
        }
        let warn_mass = "Warn: missing ID argument. This will change your mass.";
        match command.name {
            "help" => {
                if let Some(wanted) = args.get(1) {
                    if let Some(c) = CHAT_COMMANDS.iter().find(|c| c.name == *wanted) {
                        self.tell(from, &format!("usage: /{} {}", c.name, c.usage));
                        return;
                    }
                }
                let line = "~".repeat(70);
                self.tell(from, &line);
                for c in CHAT_COMMANDS.iter().filter(|c| role >= c.min_role) {
                    self.tell(from, &format!("/{} - {}.", c.name, c.description));
                }
                self.tell(from, &line);
            }
            "id" => {
                self.tell(from, &format!("Your PlayerID is {}", self.player(from).pid));
            }
            "kill" => {
                if !self.player(from).is_alive() {
                    self.tell(from, "You cannot kill yourself, because you're not joined to the game!");
                    return;
                }
                for unit in 0..2 {
                    while let Some(&cell) = self.player(from).units[unit].cells.first() {
                        let (pos, radius, color) = {
                            let c = &self.cells[cell];
                            (c.pos, c.radius, c.color)
                        };
                        self.remove_cell(cell);
                        let food = self.new_cell(Kind::Food, pos, radius);
                        self.cells[food].color = color;
                        self.cells[food].no_respawn = true;
                    }
                }
                self.tell(from, "You killed yourself");
            }
            "skin" => {
                if self.player(from).is_alive() {
                    self.tell(from, "ERROR: Cannot change skin while in game!");
                    return;
                }
                let skin = args.get(1).copied().unwrap_or("");
                self.player_mut(from).set_skin(skin);
                let message = if skin.is_empty() {
                    "Your skin was removed".to_owned()
                } else {
                    format!("Your skin set to {skin}")
                };
                self.tell(from, &message);
            }
            "login" => {
                let Some(password) = args.get(1).map(|p| p.trim()).filter(|p| !p.is_empty()) else {
                    self.tell(from, "ERROR: missing password argument!");
                    return;
                };
                let ip = &self.player(from).ip;
                let user = self
                    .data
                    .users
                    .iter()
                    .find(|u| u.password == password && (u.ip == *ip || u.ip == "*"))
                    .cloned();
                let Some(user) = user else {
                    self.tell(from, "ERROR: login failed!");
                    return;
                };
                let player = self.player_mut(from);
                player.role = user.role;
                player.user_auth = Some(user.name.clone());
                self.tell(from, &format!("Login done as \"{}\"", user.name));
            }
            "logout" => {
                if role == ROLE_GUEST {
                    self.tell(from, "ERROR: not logged in");
                    return;
                }
                let player = self.player_mut(from);
                player.role = ROLE_GUEST;
                player.user_auth = None;
                self.tell(from, "Logout done");
            }
            "mass" => {
                let Some(mass) = parse_mass(args.get(1)) else {
                    self.tell(from, "ERROR: missing mass argument!");
                    return;
                };
                let Some(target) = self.command_target(from, args.get(2), warn_mass) else {
                    return;
                };
                self.set_player_mass(target, mass);
                self.tell(from, &format!("Set mass of {} to {mass}", self.player(target).name));
                if target != from {
                    self.tell(target, &format!("{} changed your mass to {mass}", self.player(from).name));
                }
            }
            "status" => {
                let humans = self.order.iter().filter(|&&s| self.player(s).is_human()).count();
                let line = "~".repeat(57);
                self.tell(from, &line);
                self.tell(
                    from,
                    &format!(
                        "Connected players: {}/{}",
                        self.order.len(),
                        self.cfg.server_max_connections
                    ),
                );
                self.tell(
                    from,
                    &format!("Players: {humans} - Bots: {}", self.order.len() - humans),
                );
                let minutes = (self.now - self.boot_time) as u64 / 60000;
                self.tell(from, &format!("Server has been running for {minutes} minutes"));
                self.tell(from, &format!("Current game mode: {}", self.mode.name()));
                self.tell(from, &format!("Update time: {:.3}ms", self.update_time_avg));
                self.tell(from, &line);
            }
            "add-minion" => {
                let Some(count) = int(args.get(1)) else {
                    self.tell(from, "ERROR: missing count argument!");
                    return;
                };
                let warning = "Warn: missing ID argument. This will give you minions.";
                let Some(target) = self.command_target(from, args.get(2), warning) else {
                    return;
                };
                let name = args.get(3..).map(|a| a.join(" ")).unwrap_or_default();
                let size = self.cfg.minion_max_start_size;
                let count = count.clamp(0, MAX_ADDED_AT_ONCE);
                for _ in 0..count {
                    self.add_minion(target, &name, Some(size * size / 100.0));
                }
                if count > 0 {
                    self.player_mut(target).has_minions = true;
                }
                self.tell(from, &format!("Added {count} minions for {}", self.player(target).name));
                if target != from {
                    self.tell(target, &format!("{} gave you {count} minions.", self.player(from).name));
                }
            }
            "rm-minion" => {
                let Some(target) = self.command_target(from, args.get(1), "") else {
                    return;
                };
                self.player_mut(target).has_minions = false;
                self.tell(from, &format!("Succesfully removed minions for {}", self.player(target).name));
                if target != from {
                    self.tell(target, &format!("{} removed all of your minions.", self.player(from).name));
                }
            }
            "killall" => {
                let count = self.kill_all();
                self.broadcast(&format!("{} killed everyone.", self.player(from).name));
                self.tell(from, &format!("You killed everyone. ({count} cells)"));
            }
            "spawnmass" => {
                let Some(mass) = parse_mass(args.get(1)) else {
                    self.tell(from, "ERROR: missing mass argument!");
                    return;
                };
                let warning = "Warn: missing ID argument. This will change your spawnmass.";
                let Some(target) = self.command_target(from, args.get(2), warning) else {
                    return;
                };
                self.player_mut(target).spawn_size = size_from_mass(mass);
                self.tell(from, &format!("Set spawnmass of {} to {mass}", self.player(target).name));
                if target != from {
                    self.tell(target, &format!("{} changed your spawn mass to {mass}", self.player(from).name));
                }
            }
            "addbot" => {
                let Some(count) = int(args.get(1)) else {
                    self.tell(from, "ERROR: missing count argument.");
                    return;
                };
                let count = count.clamp(0, MAX_ADDED_AT_ONCE);
                for _ in 0..count {
                    self.add_bot();
                }
                logger::warn(&format!("{} ADDED {count} BOTS", self.player(from).ip));
                self.tell(from, &format!("Added {count} Bots"));
            }
            "shutdown" => {
                logger::warn(&format!(
                    "SHUTDOWN REQUEST FROM {} as {}",
                    self.player(from).ip,
                    self.player(from).user_auth.as_deref().unwrap_or("?")
                ));
                self.exit_requested = Some(0);
            }
            "restart" => {
                logger::warn(&format!(
                    "RESTART REQUEST FROM {} as {}",
                    self.player(from).ip,
                    self.player(from).user_auth.as_deref().unwrap_or("?")
                ));
                self.restart();
            }
            _ => unreachable!(),
        }
    }

    fn set_player_mass(&mut self, slot: u32, mass: f64) {
        let size = size_from_mass(mass);
        let cells: Vec<u32> = self.player(slot).all_cells().collect();
        for cell in cells {
            self.cells[cell].set_size(size);
            self.update_grid(cell);
        }
    }

    fn kill_all(&mut self) -> usize {
        let mut count = 0;
        for i in 0..self.order.len() {
            let slot = self.order[i];
            count += self.player(slot).cell_count();
            self.remove_player_cells(slot);
        }
        count
    }

    pub fn console_command(&mut self, line: &str) {
        let lowered = line.to_lowercase();
        let args: Vec<&str> = lowered.split(' ').collect();
        let name = if self.console_aliases {
            CONSOLE_HELP
                .iter()
                .find(|(command, _)| alias_of(command) == args[0])
                .map_or(args[0], |(command, _)| command)
        } else {
            args[0]
        };
        match name {
            "help" => {
                let total = CONSOLE_HELP.len() * if self.console_aliases { 2 } else { 1 };
                logger::info(&format!("The server currently supports a total of {total} commands."));
                for (i, (command, description)) in CONSOLE_HELP.iter().enumerate() {
                    logger::print(&format!("{}. {command}: {description}", i + 1));
                }
            }
            "player" => self.console_player(args.get(1).copied()),
            "minion" => {
                let Some(pid) = int(args.get(1)) else {
                    logger::warn("Please provide a numerical player ID.");
                    return;
                };
                let amount = int(args.get(2)).filter(|&n| n != 0).unwrap_or(1);
                let name = args.get(3..).map(|a| a.join(" ")).unwrap_or_default();
                let found = self.order.iter().copied().find(|&s| i64::from(self.player(s).pid) == pid);
                let Some(slot) = found else {
                    logger::warn("There is no player with that ID.");
                    return;
                };
                if self.player(slot).has_minions {
                    self.player_mut(slot).has_minions = false;
                    logger::info(&format!("Removed {}'s minions.", self.player(slot).name));
                } else if !self.player(slot).connected {
                    logger::warn(&format!("{} isn't connected", self.player(slot).name));
                } else {
                    for _ in 0..amount.clamp(0, MAX_ADDED_AT_ONCE) {
                        self.add_minion(slot, &name, None);
                    }
                    self.player_mut(slot).has_minions = true;
                    logger::print(&format!("Gave {amount} minions to {}", self.player(slot).name));
                }
            }
            "addbot" => {
                let amount = int(args.get(1)).filter(|&n| n != 0).unwrap_or(1);
                let amount = amount.clamp(0, MAX_ADDED_AT_ONCE);
                for _ in 0..amount {
                    self.add_bot();
                }
                let plural = if amount > 1 { "s" } else { "" };
                logger::print(&format!(
                    "Added {amount} player bot{plural} to the game. Use the rmbot command to remove them."
                ));
            }
            "rmbot" => {
                let bots: Vec<u32> =
                    self.order.iter().copied().filter(|&s| self.player(s).is_bot()).collect();
                let amount = int(args.get(1)).filter(|&n| n != 0).unwrap_or(bots.len() as i64);
                let mut total = 0;
                for slot in bots.into_iter().take(amount.max(0) as usize) {
                    self.player_mut(slot).close_requested = true;
                    total += 1;
                }
                logger::print(&format!(
                    "Removed a total {total} bots out of the requested amount of {amount}."
                ));
            }
            "kick" => {
                if self.order.is_empty() {
                    logger::warn("The server is empty.");
                    return;
                }
                let target = args.get(1).copied().unwrap_or("");
                let pid = parse_int(target);
                let mut total = 0;
                for i in 0..self.order.len() {
                    let slot = self.order[i];
                    if target == "all" || pid == Some(i64::from(self.player(slot).pid)) {
                        self.player_mut(slot).kick(CLOSE_NO_STATUS, "");
                        total += 1;
                    }
                }
                if total > 0 {
                    let plural = if total > 1 { "s" } else { "" };
                    logger::print(&format!("Kicked {total} client{plural}."));
                } else {
                    logger::warn("Please provide an amount of bots to kick. Or provide \"all\" to kick all bots.");
                }
            }
            "killall" => {
                if self.order.is_empty() {
                    logger::warn("The server is empty.");
                    return;
                }
                self.kill_all();
                logger::print("Removed all players.");
            }
            "mass" => {
                let Some(pid) = int(args.get(1)) else {
                    logger::warn("Please provide a numerical player id.");
                    return;
                };
                let Some(mass) = parse_mass(args.get(2)) else {
                    logger::warn("Please provide a numerical mass.");
                    return;
                };
                let found = self.order.iter().copied().find(|&s| i64::from(self.player(s).pid) == pid);
                if let Some(slot) = found {
                    self.set_player_mass(slot, mass);
                    let name = &self.player(slot).name;
                    let name = if name.is_empty() { "An unnamed cell" } else { name };
                    logger::print(&format!("Set {name}'s mass to {mass}"));
                }
            }
            "exit" => {
                logger::info("Exiting server...");
                self.exit_requested = Some(int(args.get(1)).unwrap_or(0) as i32);
            }
            "stats" => {
                let clients = self.order.len();
                logger::info(&format!(
                    "Connected players: {clients} / {}",
                    self.cfg.server_max_connections
                ));
                logger::info(&format!("Clients: {clients}"));
                logger::info(&format!(
                    "Server uptime: {}",
                    (self.now - self.boot_time) as u64 / 60000
                ));
                logger::info(&format!("Update time: {:.3}ms", self.update_time_avg));
            }
            "aliases" => {
                self.console_aliases = true;
                logger::print("Aliases generated.");
            }
            _ => {}
        }
    }

    fn console_player(&self, arg: Option<&str>) {
        let Some(arg) = arg.filter(|a| !a.is_empty()) else {
            logger::warn("Usage: player <player ID || \"list\">");
            return;
        };
        if self.order.is_empty() {
            logger::warn("There are no clients currently connected.");
        }
        let pid = parse_int(arg);
        let mut slots = self.order.clone();
        slots.sort_by_key(|&slot| self.player(slot).pid);
        for slot in slots {
            let p = self.player(slot);
            if !p.connected || p.is_minion_kind() {
                continue;
            }
            if pid == Some(i64::from(p.pid)) {
                let name = if p.name.is_empty() { "none" } else { &p.name };
                logger::info(&format!("Info record for client {}: ", p.pid));
                logger::print(&format!("- isMinion: {}", p.is_minion));
                let protocol = if p.protocol == 0 { "none".to_owned() } else { p.protocol.to_string() };
                let address = if p.ip.is_empty() { "none" } else { &p.ip };
                logger::print(&format!("- protocol:  {protocol}"));
                logger::print(&format!("- remoteAddress: {address}"));
                logger::print(&format!("- spectate: {}", p.spectate));
                logger::print(&format!("- name: {name}"));
                logger::print(&format!("- cells: {}", p.cell_count()));
                logger::print(&format!("- score: {}", p.score.floor()));
                logger::print(&format!(
                    "- position: {{x: {}, y: {}}}",
                    p.center.x.floor(),
                    p.center.y.floor()
                ));
            } else if arg == "list" {
                let name = if p.name.is_empty() { "An unnamed cell" } else { &p.name };
                logger::print(&format!("ID {} | \"{name}\"", p.pid));
            }
        }
    }
}
