use std::{task::{Poll, Waker}, time::Duration, pin::Pin, cell::{RefCell}, rc::Rc, collections::BTreeMap, fs::File, path::{Path}, io::{BufRead, BufReader}, sync::atomic::{AtomicBool, self, AtomicU16, AtomicU32}, f32::consts::PI};

use anyhow::{anyhow, Context};
use futures::FutureExt;
use nalgebra::Vector3;
use fost_protocol::{Session, packet_handler::{self, PacketHandler}, Task, packets::{Packet, PacketDowncast, self}, codec::{BattleTeam, LayoutState, MoveCommand, RotateTurretCommand}, PacketDebugFilter, SimplePacketDebugFilter};
use tokio::{sync::oneshot, task};
use tracing::{info, warn};
use clap::Parser;
use tracing_subscriber::{Registry, fmt::Layer};
use tracing_subscriber::prelude::*;
use chrono::Local;

use serde::{ Deserialize, Serialize };

static SMOKY_SHOT: AtomicBool = AtomicBool::new(true);

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BattleUserInit {
    pub battle_id: String,
    #[serde(rename = "colormap_id")]
    pub colormap_id: i64,
    #[serde(rename = "hull_id")]
    pub hull_id: String,
    #[serde(rename = "turret_id")]
    pub turret_id: String,
    #[serde(rename = "team_type")]
    pub team_type: String,
    pub parts_object: String,
    pub hull_resource: i64,
    pub turret_resource: i64,
    pub sfx_data: String,
    pub position: JsonVec3f,
    pub orientation: JsonVec3f,
    pub incarnation: i16,
    #[serde(rename = "tank_id")]
    pub tank_id: String,
    pub nickname: String,
    pub state: String,
    pub max_speed: f32,
    pub max_turn_speed: f64,
    pub acceleration: f64,
    pub reverse_acceleration: f64,
    pub side_acceleration: f64,
    pub turn_acceleration: f64,
    pub reverse_turn_acceleration: f64,
    pub mass: f32,
    pub power: f64,
    pub damping_coeff: f32,
    #[serde(rename = "turret_turn_speed")]
    pub turret_turn_speed: f64,
    pub health: f32,
    pub rank: i64,
    pub kickback: f32,
    pub turret_turn_acceleration: f64,
    #[serde(rename = "impact_force")]
    pub impact_force: f32,
    #[serde(rename = "state_null")]
    pub state_null: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonVec3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Into<Vector3<f32>> for JsonVec3f {
    fn into(self) -> Vector3<f32> {
        Vector3::new(self.x, self.y, self.z)
    }
}

struct TaskSimpleAction<
    E: FnOnce(&mut Session) -> anyhow::Result<()>,
    M: Fn(&mut Session, &dyn Packet) -> anyhow::Result<Option<R>>,
    R: Send
> {
    executor: Option<E>,
    matcher: M,

    tx: Option<oneshot::Sender<anyhow::Result<R>>>,
    rx: oneshot::Receiver<anyhow::Result<R>>,
}

impl<
    E: (FnOnce(&mut Session) -> anyhow::Result<()>) + Send,
    M: (Fn(&mut Session, &dyn Packet) -> anyhow::Result<Option<R>>) + Send,
    R: Send
> TaskSimpleAction<E, M, R> {
    pub fn create(executor: E, matcher: M) -> Self {
        let (tx, rx) = oneshot::channel();
        Self {
            executor: Some(executor),
            matcher,

            tx: Some(tx),
            rx,
        }
    }

    fn submit_result(&mut self, result: anyhow::Result<R>) -> anyhow::Result<()> {
        let sender = match self.tx.take() {
            Some(sender) => sender,
            None => anyhow::bail!("missing result sender")
        };

        if sender.send(result).is_err() {
            anyhow::bail!("failed to submit task result");
        }
        Ok(())
    }
}

impl<
    E: (FnOnce(&mut Session) -> anyhow::Result<()>) + Send,
    M: (Fn(&mut Session, &dyn Packet) -> anyhow::Result<Option<R>>) + Send,
    R: Send
> Task for TaskSimpleAction<E, M, R> {
    type Result = R;

    fn handle_packet(&mut self, client: &mut Session, packet: &dyn Packet) -> anyhow::Result<()> {
        if self.executor.is_some() {
            return Ok(())
        }

        if self.tx.is_none() {
            return Ok(())
        }

        match (self.matcher)(client, packet) {
            Ok(Some(result)) => {
                self.submit_result(Ok(result))?;
            },
            Ok(None) => {}
            Err(error) => {
                self.submit_result(Err(error))?;
            }
        }

        Ok(())
    }

    fn poll(&mut self, client: &mut Session, cx: &mut std::task::Context) -> std::task::Poll<anyhow::Result<Self::Result>> {
        if let Some(executor) = self.executor.take() {
            (executor)(client)?;
        }

        match self.rx.poll_unpin(cx) {
            Poll::Ready(Ok(result)) => Poll::Ready(result),
            Poll::Ready(Err(_)) => Poll::Ready(Err(anyhow!("failed to poll matcher result"))),
            Poll::Pending => Poll::Pending
        }
    }
}

#[derive(Debug)]
enum LoginResult {
    Success,
    Falure,
    BanTemporary { reason: String },
    BanPermanent { reason: String },
}

struct TaskAccountLogin;
impl TaskAccountLogin {
    pub fn new(username: String, password: String) -> impl Task<Result = LoginResult> {
        TaskSimpleAction::create(
            |client| {
                client.connection.send_packet(&packets::C2SAccountLoginExecute{
                    login: username,
                    password,
                    remember: false
                })?;

                Ok(())
            }, 
            |_, packet| {
                if packet.is_type::<packets::S2CAccountLoginFailure>() {
                    Ok(Some(LoginResult::Falure))
                } else if packet.is_type::<packets::S2CAccountLoginSuccess>() {
                    Ok(Some(LoginResult::Success))
                } else if let Some(packet) = packet.downcast_ref::<packets::S2CBanPermanent>() {
                    Ok(Some(LoginResult::BanPermanent { reason: packet.reason_for_user.clone() }))
                } else if let Some(packet) = packet.downcast_ref::<packets::S2CBanTemporary>() {
                    Ok(Some(LoginResult::BanTemporary { reason: packet.reason_for_user.clone() }))
                } else {
                    Ok(None)
                }
            }
        )
    }
}

struct TaskBattleList;
impl TaskBattleList {
    pub fn join_selected_battle(team: BattleTeam) -> impl Task<Result = ()> {
        TaskSimpleAction::create(
            move |client| {
                client.connection.send_packet(&packets::S2CBattleInfoJoinBattle {
                    var_625: team,
                })?;
                Ok(())
            }, 
            |_, packet| {
                if let Some(packet) = packet.downcast_ref::<packets::S2CLobbyLayoutSwitchStart>() {
                    if packet.state == LayoutState::Battle {
                        return Ok(Some(()))
                    }
                }
                Ok(None)
            }
        )
    } 

    pub fn select_battle(battle_id: String) -> impl Task<Result = bool> {
        let battle_id2 = battle_id.clone();
        TaskSimpleAction::create(
            move |client| {
                client.connection.send_packet(&packets::C2SBattleListBattleSelect {
                    item: battle_id2
                })?;
                Ok(())
            }, 
            move |_, packet| {
                if let Some(packet) = packet.downcast_ref::<packets::S2CBattleListBattleSelect>() {
                    if packet.item == battle_id {
                        return Ok(Some(true))
                    }
                } else if let Some(packet) = packet.downcast_ref::<packets::S2CLinkResultDead>() {
                    if packet.battle_id == battle_id {
                        return Ok(Some(false))
                    }
                }
                
                Ok(None)
            }
        )
    }
}

#[derive(Parser, Debug, Clone)]
struct Args {
    /// Target server address
    #[arg(short, long)]
    target: String,

    #[arg(short, long)]
    userlist: String,

    #[arg(short, long)]
    battle_id: String,
    
    /// Target language code
    #[arg(long, default_value = "en")]
    language_code: String,
    
    /// Target language code
    #[arg(long)]
    log_protocol: bool,
}

struct LogFilter;
impl PacketDebugFilter for LogFilter {
    fn should_log(&self, _is_send: bool, packet: &dyn Packet) -> bool {
        match packet.model_id() {
            45 => false, /* low level ping */
            32 => false, /* battle list */
            _ => true
        }
    }
}

struct PacketHandlerRandomMoveControlFlags {
    tank_id: String,
    interval: Pin<Box<tokio::time::Interval>>
}
impl PacketHandlerRandomMoveControlFlags {
    pub fn new(tank_id: String, period: Duration) -> Self {
        Self {
            tank_id,
            interval: Box::pin(
                tokio::time::interval(period)
            )
        }
    }
}
impl PacketHandler for PacketHandlerRandomMoveControlFlags {
    fn poll(&mut self, client: &mut Session, cx: &mut std::task::Context) -> Poll<anyhow::Result<()>> {
        while let Poll::Ready(_) = self.interval.poll_tick(cx) {
            let tanks = match client.get_component::<BattleTanks>() {
                Some(tanks) => tanks,
                None => continue,
            };

            let tank = match tanks.tanks.get(&self.tank_id) {
                Some(tank) => tank,
                None => continue,
            };

            if tank.state == TankState::Dead {
                client.connection.send_packet(&packets::C2STankMoveControlFlags{
                    control: 0,
                    name_43: client.session_timestamp(),
                    specification_id: tank.incarnation_id
                })?;
                continue;
            }

            client.connection.send_packet(&packets::C2STankTurretCommand{
                incarnation_id: tank.incarnation_id,
                name_43: client.session_timestamp(),
                rotate_turret_command: RotateTurretCommand { 
                    target: rand::random::<f32>() * PI * 2f32,
                    control: if rand::random::<bool>() { 32 } else { 64 } 
                }
            })?;
        }

        Poll::Pending
    }
}

enum LocalTankState {
    Uninit,
    Dead{ timer: Pin<Box<tokio::time::Sleep>> },
    Placed{ timer: Pin<Box<tokio::time::Sleep>> },
    Activated
}

struct PacketHandlerTankSpawner {
    local_id: String,
    state: Rc<RefCell<LocalTankState>>,
    waker: Option<Waker>,
}

impl PacketHandlerTankSpawner {
    pub fn new(local_id: String) -> Self {
        Self {
            local_id,
            state: Rc::new(RefCell::new(LocalTankState::Uninit)),
            waker: None
        }
    }

    fn update(&mut self, state: &mut LocalTankState, client: &mut Session, cx: &mut std::task::Context) -> anyhow::Result<Option<LocalTankState>> {
        let new_state = match state {
            LocalTankState::Uninit => {
                client.connection.send_packet(&packets::C2STankInit{})?;
                Some(
                    LocalTankState::Dead { 
                        timer: Box::pin(
                            tokio::time::sleep(Duration::from_secs(3))
                        )
                    }
                )
            },
            LocalTankState::Dead { timer } => {
                if timer.poll_unpin(cx).is_ready() {
                    client.connection.send_packet(&packets::C2STankReady2Place{})?;

                    Some(
                        LocalTankState::Placed { 
                            timer: Box::pin(
                                tokio::time::sleep(Duration::from_millis(2500))
                            )
                        }
                    )
                } else {
                    None
                }
            },
            LocalTankState::Placed { timer } => {
                if timer.poll_unpin(cx).is_ready() {
                    client.connection.send_packet(&packets::C2STankReady2Activate{})?;
                    Some(LocalTankState::Activated)
                } else {
                    None
                }
            },
            _ => None
        };

        Ok(new_state)
    }
}

impl PacketHandler for PacketHandlerTankSpawner {
    fn handle_packet(&mut self, _client: &mut Session, packet: &dyn Packet) -> anyhow::Result<()> {
        if let Some(packet) = packet.downcast_ref::<packets::S2CTankKill>() {
            if packet.tank_id == self.local_id {
                info!("Own tank died. Respawning.");
                /* The respwan interval is currently not checked */
                *self.state.borrow_mut() = LocalTankState::Dead { 
                    timer: Box::pin(
                        //tokio::time::sleep(Duration::from_millis(packet.respawn_delay as u64))
                        tokio::time::sleep(Duration::from_millis(0))
                    )
                };

                if let Some(waker) = self.waker.take() {
                    waker.wake();
                }
            }
        }

        Ok(())    
    }

    fn poll(&mut self, client: &mut Session, cx: &mut std::task::Context) -> Poll<anyhow::Result<()>> {
        let state = self.state.clone();
        let mut state = state.borrow_mut();
        while let Some(new_state) = self.update(&mut *state, client, cx)? {
            *state = new_state;
        }
        
        self.waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

struct PacketHandlerTankSmokyShoter {
    local_tank_id: String,
    interval: Pin<Box<tokio::time::Interval>>
}

impl PacketHandlerTankSmokyShoter {
    pub fn new(local_tank_id: String, period: Duration) -> Self {
        Self {
            local_tank_id,
            interval: Box::pin(
                tokio::time::interval(period)
            )
        }
    }
}

impl PacketHandler for PacketHandlerTankSmokyShoter {
    fn poll(&mut self, client: &mut Session, cx: &mut std::task::Context) -> Poll<anyhow::Result<()>> {
        while let Poll::Ready(_) = self.interval.poll_tick(cx) {
            if !SMOKY_SHOT.load(atomic::Ordering::Relaxed) {
                continue;
            }

            let tanks = match client.get_component::<BattleTanks>() {
                Some(tanks) => tanks,
                None => continue,
            };

            let local_tank = match tanks.tanks.get(&self.local_tank_id) {
                Some(tank) => tank,
                None => continue
            };

            if local_tank.state != TankState::Active {
                continue;
            }

            let mut best_tank = None;
            let mut best_distance = 1e9;
            for tank in tanks.tanks.values() {
                if tank.tank_id == local_tank.tank_id {
                    continue;
                }

                if tank.state != TankState::Active {
                    continue;
                }

                if tank.team != BattleTeam::None && tank.team == local_tank.team {
                    continue;
                }

                let distance = (tank.position - local_tank.position).norm();
                if distance < best_distance {
                    best_tank = Some(tank);
                    best_distance = distance;
                }
            }

            if let Some(tank) = best_tank {
                client.connection.send_packet(&packets::C2SWeaponSmokyShot{
                    name_43: client.session_timestamp(),
                    target: tank.tank_id.clone(),
                    var_356: tank.incarnation_id,

                    hit_point: Some(Vector3::zeros()),
                    var_253: Some(Vector3::zeros()),
                    var_2967: Some(Vector3::zeros()),
                })?;
            }
        }
        
        Poll::Pending
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TankState {
    NewCome,
    Active,
    Dead
}

impl Default for TankState {
    fn default() -> Self {
        TankState::NewCome
    }
}

#[derive(Debug, Default)]
struct BattleTank {
    pub tank_id: String,
    pub team: BattleTeam,

    pub health: f32,
    pub incarnation_id: i16,
    pub state: TankState,

    pub control: u8,

    pub position: Vector3<f32>,
    pub orientation: Vector3<f32>,
    pub velocity: Vector3<f32>,
    pub angular_velocity: Vector3<f32>,

    pub turret_rotation: f32,
}

impl BattleTank {
    fn update_from_move_command(&mut self, command: &MoveCommand) -> anyhow::Result<()> {
        self.position = command.position.context("missing position")?;
        self.velocity = command.velocity.context("missing position")?;
        self.angular_velocity = command.angular_velocity.context("missing position")?;
        self.orientation = command.orientation.context("missing position")?;

        Ok(())
    }

    fn update_from_turret_command(&mut self, command: &RotateTurretCommand) {
        self.turret_rotation = command.target;
    }
}

struct BattleTanks {
    pub local_tank_id: String,
    pub tanks: BTreeMap<String, Box<BattleTank>>,
}

impl BattleTanks {
    fn new(local_tank_id: String) -> Self {
        Self {
            local_tank_id,
            tanks: Default::default(),
        }
    }
}

struct BattleTanksPacketHandler;
impl PacketHandler for BattleTanksPacketHandler {
    fn handle_packet(&mut self, client: &mut Session, packet: &dyn Packet) -> anyhow::Result<()> {
        let tanks = match client.get_component_mut::<BattleTanks>() {
            Some(tanks) => tanks,
            None => anyhow::bail!("missing battle tanks component")
        };
        
        if let Some(packet) = packet.downcast_ref::<packets::S2CBattleUserInit>() {
            let payload = serde_json::from_str::<BattleUserInit>(&packet.json)?;
            
            let tank = BattleTank{
                tank_id: payload.tank_id.clone(),
                team: match payload.team_type.as_str() {
                    "RED" => BattleTeam::Red,
                    "BLUE" => BattleTeam::Blue,
                    "NONE" => BattleTeam::None,
                    value => anyhow::bail!("invalid tank team: {}", value)
                },
                state: match payload.state.as_str() {
                    "newcome" => TankState::NewCome,
                    "active" => TankState::Active,
                    "suicide" => TankState::Dead,
                    value => anyhow::bail!("invalid tank state: {}", value)
                },

                health: payload.health,
                incarnation_id: payload.incarnation,

                position: payload.position.into(),
                orientation: payload.orientation.into(),

                ..BattleTank::default()
            };

            if let Some(_) = tanks.tanks.insert(payload.tank_id.clone(), Box::new(tank)) {
                anyhow::bail!("failed to init user tank as it already existed");
            }
        } else if let Some(packet) = packet.downcast_ref::<packets::S2CTankSpawn>() {
            let tank = BattleTank{
                tank_id: packet.tank_id.clone(),
                team: packet.team.clone(),

                health: packet.health as f32,
                incarnation_id: packet.incarnation_id,

                position: packet.position.context("missing position")?,
                orientation: packet.orientation.context("missing position")?,

                ..BattleTank::default()
            };

            if let Some(old_tank) = tanks.tanks.insert(packet.tank_id.clone(), Box::new(tank)) {
                if old_tank.state != TankState::Dead {
                    warn!("Received new task for {} but old tank still existed.", &packet.tank_id);
                }
            }
        } else if let Some(packet) = packet.downcast_ref::<packets::S2CTankHealth>() {
            if let Some(tank) = tanks.tanks.get_mut(&packet.tank_id) {
                tank.health = packet.health;
            }
        } else if let Some(packet) = packet.downcast_ref::<packets::S2CTankActivated>() {
            if let Some(tank) = tanks.tanks.get_mut(&packet.tank_id) {
                tank.state = TankState::Active;
            }
        } else if let Some(packet) = packet.downcast_ref::<packets::S2CTankKill>() {
            if let Some(tank) = tanks.tanks.get_mut(&packet.tank_id) {
                tank.state = TankState::Dead;
            }
        } else if let Some(packet) = packet.downcast_ref::<packets::S2CTankMoveControlFlags>() {
            if let Some(tank) = tanks.tanks.get_mut(&packet.tank_id) {
                tank.control = packet.control as u8;
            }
        } else if let Some(packet) = packet.downcast_ref::<packets::S2CTankMoveTurretCommand>() {
            if let Some(tank) = tanks.tanks.get_mut(&packet.tank_id) {
                tank.update_from_move_command(&packet.move_command)?;
                tank.turret_rotation = packet.turret_direction;
            }
        } else if let Some(packet) = packet.downcast_ref::<packets::S2CTankMoveCommand>() {
            if let Some(tank) = tanks.tanks.get_mut(&packet.tank_id) {
                tank.update_from_move_command(&packet.move_command)?;
            }
        } else if let Some(packet) = packet.downcast_ref::<packets::S2CTankUpdateOrientation>() {
            let local_tank = tanks.tanks.get_mut(&tanks.local_tank_id).context("missing local tank")?;
            local_tank.position = packet.position.context("missing tank position")?;
            local_tank.orientation = packet.orientation.context("missing tank orientation")?;
        } else if let Some(packet) = packet.downcast_ref::<packets::S2CTankTurretCommand>() {
            if let Some(tank) = tanks.tanks.get_mut(&packet.tank_id) {
                tank.update_from_turret_command(&packet.rotate_turret_command);
            }
        } else if let Some(packet) = packet.downcast_ref::<packets::S2CTankDestroy>() {
            tanks.tanks.remove(&packet.tank);
        }
        Ok(())
    }

    fn poll(&mut self, _client: &mut Session, _cx: &mut std::task::Context) -> Poll<anyhow::Result<()>> {
        Poll::Pending
    }
}

static RED_COUNT: AtomicU32 = AtomicU32::new(0);
async fn create_shot_bot(args: &Args, username: String, password: String, battle_id: String) -> anyhow::Result<()> {
    let mut client = Session::builder()
        .set_lang_code(args.language_code.clone())
        .set_log_filter(if args.log_protocol { Box::new(LogFilter{}) } else { Box::new(SimplePacketDebugFilter::logging_disabled()) })
        .connect(args.target.parse()?).await?;

    client.register_packet_handler(packet_handler::DummyResourceLoader{});
    client.register_packet_handler(packet_handler::LowLevelPing{});
    client.register_packet_handler(packet_handler::SessionPing{});

    info!("Client connected.");

    client.await_server_resources_loaded().await?;
    info!("Client loaded and viewing the login screen.");

    let login_result = client.execute_task(
        TaskAccountLogin::new(username.clone(), password.clone())
    ).await?;
    match login_result {
        LoginResult::Success => {},
        result => {
            anyhow::bail!("login failed: {:?}", result);
        }
    }

    client.await_match(|_, packet| {
        if let Some(_) = packet.downcast_ref::<packets::S2CLobbyLayoutSwitchEnd>() {
            Some(())
        } else {
            None
        }
    }).await?;

    client.connection.send_packet(&packets::C2SGarageMountItem{
        item: "smoky_m0".to_string()
    })?;
    client.connection.send_packet(&packets::C2SGarageBuyItem{
        item: "wasp_m0".to_string(),
        count: 1,
        var_204: 120
    })?;
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_millis(1000)) => {},
        _ = &mut client => {}
    };
    client.connection.send_packet(&packets::C2SGarageMountItem{
        item: "wasp_m0".to_string()
    })?;
    let success = client.execute_task(
        TaskBattleList::select_battle(battle_id.clone())
    ).await?;
    if !success {
        anyhow::bail!("failed to select target battle")
    }

    info!("joining battle!");
    client.execute_task(
        TaskBattleList::join_selected_battle(if RED_COUNT.fetch_add(1, atomic::Ordering::Relaxed) % 2 == 0 { BattleTeam::Red } else { BattleTeam::Blue })
    ).await?;

    client.register_component(BattleTanks::new(username.clone()));
    client.register_packet_handler(PacketHandlerRandomMoveControlFlags::new(username.clone(), Duration::from_millis(1000)));
    client.register_packet_handler(BattleTanksPacketHandler{});

    client.await_match(|_, packet| {
        if let Some(packet) = packet.downcast_ref::<packets::S2CBattleMapInfo>() {
            Some(packet.json.clone())
        } else {
            None
        }
    }).await?;
    info!("received map");

    /* await switch finish */
    client.await_match(|_, packet| if packet.is_type::<packets::S2CLobbyLayoutSwitchEnd>() { Some(()) } else { None }).await?;

    info!("sending ready & spawn packets");
    client.register_packet_handler(PacketHandlerTankSpawner::new(username.clone()));
    client.register_packet_handler(PacketHandlerTankSmokyShoter::new(username.clone(), Duration::from_millis(1900)));

    client.await;

    info!("Client disconnected.");
    Ok(())
}

#[derive(Debug, Default)]
struct UserList {
    users: Vec<(String, String)>,
}

impl UserList {
    pub fn from_file(file: &Path) -> anyhow::Result<Self> {
        let file = File::open(file)?;
        let reader = BufReader::new(file);
        let mut result = Self::default();
        for line in reader.lines() {
            let line = line?;
            if line.starts_with(";") {
                continue;
            } else if line.starts_with("--- END") {
                break;
            }

            let (username, password) = line.split_once(":").context("invalid user entry")?.to_owned();
            result.users.push((username.to_string(), password.to_string()));
        }

        Ok(result)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let log_file_name = format!("{}_crystal-bot.log", Local::now().format("%d-%m-%d_%H-%M-%S"));
    let subscriber = Registry::default()
        .with(
            Layer::default()
                .with_ansi(false)
                .with_writer(tracing_appender::rolling::never("logs", log_file_name))
        )
        .with(
            Layer::default()
                .with_writer(std::io::stdout)
        );
    tracing::subscriber::set_global_default(subscriber)?;

    let args: Args = Args::parse();
    let users = UserList::from_file(&Path::new(&args.userlist)).context("failed to load users")?;

    let mut local = task::LocalSet::new();
    for (username, password) in users.users {
        let args = args.clone();
        local.spawn_local(async move {
            loop {
                if let Err(error) = create_shot_bot(&args, username.clone(), password.clone(), args.battle_id.to_owned()).await {
                    if format!("{:?}", error).contains("BanPermanent") {
                        tracing::warn!("Perma ban: {:?}", error);
                        break;
                    }
                    tracing::error!("{}: {:?}", username, error);
                }
            }
        });

        /* sleep a little so not everything is in sync */
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(500)) => {},
            _ = &mut local => {}
        };
    }

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = &mut local => {}
    };
    SMOKY_SHOT.store(false, atomic::Ordering::Relaxed);
    info!("Shot stopped");
    
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = &mut local => {}
    };
    Ok(())
}
