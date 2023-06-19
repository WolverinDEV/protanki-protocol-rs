use std::{sync::{Arc, RwLock}, vec};

use fost_protocol::packets::s2c;
use serde::{Serialize, Deserialize};

use crate::{BattleProvider, client::ClientComponent};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BattleInfo {
    pub battle_id: String,
    pub battle_mode: String,

    pub map: String,
    pub max_people: i64,

    pub name: String,

    pub private_battle: bool,
    pub pro_battle: bool,
    pub parkour_mode: bool,
    pub equipment_constraints_mode: String,
    
    pub min_rank: i64,
    pub max_rank: i64,
    
    pub preview: i64,
    pub suspicion_level: String,
    
    #[serde(default)]
    pub users_blue: Vec<String>,
    #[serde(default)]
    pub users_red: Vec<String>,
    #[serde(default)]
    pub users: Vec<String>,
}

lazy_static::lazy_static!{
    static ref DUMMY_BATTLE: BattleInfo = BattleInfo{
        battle_id: "0000000000000000".into(),
        battle_mode: "TDM".into(),

        name: "Dummy Battle".into(),
        map: "map_silence_moon".into(),

        preview: 952789,

        equipment_constraints_mode: "NONE".into(),
        suspicion_level: "NONE".into(),

        max_people: 4,
        min_rank: 0,
        max_rank: 21,

        ..Default::default()
    };
}

pub struct ClientBattleList {
    battle_provider: Arc<RwLock<BattleProvider>>
}

impl ClientBattleList {
    pub fn new(battle_provider: Arc<RwLock<BattleProvider>>) -> Self {
        Self {
            battle_provider
        }
    }
}

impl ClientComponent for ClientBattleList {
    fn initialize(&mut self, client: &mut crate::client::Client) -> anyhow::Result<()> {
        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct BattleList {
            pub battles: Vec<BattleInfo>,
        }

        client.send_packet(&s2c::BattleListListCreate{
            json: serde_json::to_string(&BattleList{
                battles: vec![
                    DUMMY_BATTLE.clone()
                ]
            })?
        });
        Ok(())
    }
}

/// Client handler for subscribing a battle
pub struct ClientBattleInfo {}

static MAPS_JSON: &'static str = include_str!("../../resources/maps.json");

/// Client handler for creating new battles
pub struct ClientBattleCreate {
    battle_provider: Arc<RwLock<BattleProvider>>
}

impl ClientBattleCreate {
    pub fn new(battle_provider: Arc<RwLock<BattleProvider>>) -> Self {
        Self {
            battle_provider
        }
    }
}

impl ClientComponent for ClientBattleCreate {
    fn initialize(&mut self, client: &mut crate::client::Client) -> anyhow::Result<()> {
        client.send_packet(&s2c::BattleCreateParameters{
            json: MAPS_JSON.to_string()
        });
        Ok(())
    }
}