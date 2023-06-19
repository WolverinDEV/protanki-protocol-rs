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