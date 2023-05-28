#![allow(non_camel_case_types)]
use std::fmt::Debug;

use nalgebra::Vector3;

use crate::{codec::Codeable, resources};
use crate::ProtocolResult;

use super::{Achievement, BattleMode, EquipmentConstraintsMode, MapTheme, ChatModeratorLevel, ControlPointState, ItemCategory, ItemViewCategory, DamageIndicatorType};

macro_rules! codec_struct {
    ($name:ident { $($field:ident: $type:ty)* }) => {
        #[derive(Default, Clone, Debug)]
        pub struct $name {
            $(pub $field: $type),*
        }

        impl Codeable for $name {
            fn encode(&self, writer: &mut dyn std::io::Write) -> ProtocolResult<()> {
                $(self.$field.encode(writer)?;)*
                Ok(())
            }

            fn decode(&mut self, reader: &mut dyn std::io::Read) -> ProtocolResult<()> {
                $(self.$field.decode(reader)?;)*
                Ok(())
            }
        }
    };
}

#[derive(Default, Clone)]
pub struct ResourceReference {
    pub resource_id: u32
}
impl Codeable for ResourceReference {
    fn encode(&self, writer: &mut dyn std::io::Write) -> ProtocolResult<()> {
        self.resource_id.encode(writer)
    }

    fn decode(&mut self, reader: &mut dyn std::io::Read) -> ProtocolResult<()> {
        self.resource_id.decode(reader)
    }
}
impl Debug for ResourceReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceReference")
            .field("resource_id", &self.resource_id)
            .field("url", &resources::build_resource_path(self.resource_id as u64, 1))
            .finish()
    }
}

codec_struct!(class_16 {
    uid: Option<String>
    user_id: Option<String>
});

codec_struct!(class_14 {
    bonus_id: Option<String>
    name_38: i32
    method_219: Option<Vector3<f32>>
});

codec_struct!(WeeklyQuestRewardItem {
    count: i32
    method_2511: ResourceReference
});

codec_struct!(WeeklyQuestDescription {
    method_2704: i32
    name_23: i32
    method_798: bool
    name_35: ResourceReference
    name_61: ResourceReference
});

codec_struct!(UserStatus {
    chat_moderator_level: ChatModeratorLevel
    ip: Option<String>
    rank_index: i32
    uid: Option<String>
});

codec_struct!(UserStat {
    deaths: i32
    kills: i32
    score: i32
    user: Option<String>
});

codec_struct!(UserReward {
    name_6: i32
    name_59: i32
    reward: i32
    user_id: Option<String>
});

codec_struct!(UserPropertyCC {
    crystals: i32
    current_rank_score: i32
    duration_crystal_abonement: i32
    has_double_crystal: bool
    next_rank_score: i32
    place: i32
    rank: i8
    rating: f32
    score: i32
    server_number: i32
    id: Option<String>
    user_profile_url: Option<String>
});

codec_struct!(UserInfo {
    chat_moderator_level: ChatModeratorLevel
    deaths: i32
    kills: i32
    rank: i8
    score: i32
    uid: Option<String>
});

codec_struct!(UserContainerCC {
    users: Option<Vec<Option<String>>>
});

codec_struct!(TipItemCC {
    preview: ResourceReference
});

codec_struct!(TargetTankDamage {
    method_2673: f32
    method_2351: DamageIndicatorType
    target: Option<String>
});

codec_struct!(TargetPosition {
    name_22: Option<Vector3<f32>>
    orientation: Option<Vector3<f32>>
    position: Option<Vector3<f32>>
    turret_angle: f32
});

codec_struct!(TargetHit {
    direction: Option<Vector3<f32>>
    name_22: Option<Vector3<f32>>
    method_1131: i8
});

codec_struct!(StringPair {
    key: Option<String>
    value: Option<String>
});

codec_struct!(StatisticsTeamCC {
    method_1860: i32
    method_2648: i32
    method_1840: Vec<UserInfo>
    method_1572: Vec<UserInfo>
});

codec_struct!(StatisticsModelCC {
    battle_mode: BattleMode
    equipment_constraints_mode: EquipmentConstraintsMode
    fund: i32
    method_1309: BattleLimits
    map_name: Option<String>
    max_people_count: i32
    parkour_mode: bool
    method_2682: i32
    spectator: bool
    method_2378: Option<Vec<Option<String>>>
    name_5: i32
});

codec_struct!(StatisticsDMCC {
    users_info: Vec<UserInfo>
});

codec_struct!(SocialNetworkPanelParams {
    authorization_url: Option<String>
    link_exists: bool
    sn_id: Option<String>
});

codec_struct!(SocialNetworkPanelCC {
    password_created: bool
    social_network_params: Vec<SocialNetworkPanelParams>
});

codec_struct!(RotateTurretCommand {
    angle: f32
    control: i8
});

codec_struct!(RankNotifierData {
    rank: i32
    user_id: Option<String>
});

codec_struct!(Range {
    max: i32
    min: i32
});

codec_struct!(PremiumNotifierData {
    premium_time_left_in_seconds: i32
    user_id: Option<String>
});

codec_struct!(PremiumNotifierCC {
    life_time_in_seconds: i32
});

codec_struct!(PremiumAccountAlertCC {
    need_show_notification_completion_premium: bool
    need_show_welcome_alert: bool
    reminder_completion_premium_time: f32
    was_show_alert_for_first_purchase_premium: bool
    was_show_reminder_completion_premium: bool
});

codec_struct!(OnlineNotifierData {
    online: bool
    server_number: i32
    user_id: Option<String>
});

codec_struct!(NewsShowingCC {
    news_items: Vec<NewsItemCC>
});

codec_struct!(NewsItemCC {
    image_url: Option<String>
    news_date: Option<String>
    news_text: Option<String>
});

codec_struct!(MoveCommand {
    angular_velocity: Option<Vector3<f32>>
    control: i8
    method_1323: Option<Vector3<f32>>
    orientation: Option<Vector3<f32>>
    position: Option<Vector3<f32>>
});

codec_struct!(LocaleStruct {
    images: Vec<ImagePair>
    strings: Vec<StringPair>
});

codec_struct!(ImagePair {
    key: Option<String>
    value: Vec<u8>
});

codec_struct!(GarageItemInfo {
    category: ItemCategory
    item_view_category: ItemViewCategory
    modification_index: i32
    mounted: bool
    name: Option<String>
    position: i32
    premium_item: bool
    preview: ResourceReference
    remaing_time_in_ms: i32
});

codec_struct!(DominationSounds {
    method_744: ResourceReference
    method_491: ResourceReference
    method_2387: ResourceReference
    method_2280: ResourceReference
    method_793: ResourceReference
    method_656: ResourceReference
    method_402: ResourceReference
    method_95: ResourceReference
    method_383: ResourceReference
    method_1969: ResourceReference
});

codec_struct!(DominationResources {
    method_1141: ResourceReference
    method_2672: ResourceReference
    method_2738: ResourceReference
    method_2138: ResourceReference
    method_1045: ResourceReference
    method_2096: ResourceReference
    method_2753: ResourceReference
    name_65: ResourceReference
    method_2099: ResourceReference
    method_1098: ResourceReference
    method_925: ResourceReference
    method_547: ResourceReference
});

codec_struct!(DailyQuestPrizeInfo {
    count: i32
    name: Option<String>
});

codec_struct!(DailyQuestInfo {
    method_2718: bool
    description: Option<String>
    method_2630: i32
    image: ResourceReference
    method_562: Vec<DailyQuestPrizeInfo>
    progress: i32
    method_2366: i32
});

codec_struct!(ControlPointsCC {
    method_337: f32
    name_47: f32
    method_1123: f32
    name_43: Vec<ClientPointData>
    resources: DominationResources
    name_8: DominationSounds
});

codec_struct!(ClientPointData {
    id: i32
    name: Option<String>
    position: Option<Vector3<f32>>
    score: f32
    method_2190: f32
    state: ControlPointState
    method_2697: Option<Vec<Option<String>>>
});

codec_struct!(ClientFlag {
    method_1384: Option<Vector3<f32>>
    method_1275: Option<String>
    name_81: Option<Vector3<f32>>
});

codec_struct!(ClientAssaultFlag {
    method_1384: Option<Vector3<f32>>
    method_1275: Option<String>
    name_81: Option<Vector3<f32>>
    id: i32
});

codec_struct!(ChatMessage {
    source_user_status: Option<UserStatus>
    system: bool
    target_user_status: Option<UserStatus>
    text: Option<String>
    warning: bool
});

codec_struct!(ChatCC {
    admin: bool
    antiflood_enabled: bool
    buffer_size: i32
    chat_enabled: bool
    chat_moderator_level: ChatModeratorLevel
    links_white_list: Option<Vec<Option<String>>>
    min_char: i32
    min_word: i32
    self_name: Option<String>
    show_links: bool
    typing_speed_antiflood_enabled: bool
});

codec_struct!(CaptureTheFlagSoundFX {
    name_55: ResourceReference
    name_79: ResourceReference
    name_37: ResourceReference
    name_71: ResourceReference
});

codec_struct!(CaptureTheFlagCC {
    method_2047: ClientFlag
    method_1345: ResourceReference
    method_1814: ResourceReference
    method_1229: ClientFlag
    method_1505: ResourceReference
    method_872: ResourceReference
    name_8: CaptureTheFlagSoundFX
});

codec_struct!(BonusInfoCC {
    bottom_text: Option<String>
    image: ResourceReference
    top_text: Option<String>
});

codec_struct!(BattleNotifierData {
    battle_data: BattleInfoData
    user_id: Option<String>
});

codec_struct!(BattleMineCC {
    method_1937: ResourceReference
    method_1406: i32
    method_2407: Vec<BattleMine>
    method_2285: ResourceReference
    method_2634: ResourceReference
    method_2393: ResourceReference
    explosion_mark_texture: ResourceReference
    explosion_sound: ResourceReference
    method_2024: f32
    method_2226: ResourceReference
    method_1764: ResourceReference
    impact_force: f32
    method_2618: ResourceReference
    name_45: f32
    method_2145: ResourceReference
    method_472: f32
    radius: f32
    method_1957: ResourceReference
});

codec_struct!(BattleMine {
    mine_id: Option<String>
    owner_id: Option<String>
    position: Option<Vector3<f32>>
});

codec_struct!(BattleLimits {
    score_limit: i32
    time_limit_in_sec: i32
});

codec_struct!(BattleInviteMessage {
    available_rank: bool
    available_slot: bool
    battle_id: Option<String>
    map_name: Option<String>
    mode: BattleMode
    no_supplies_battle: bool
    private_battle: bool
});

codec_struct!(BattleInviteCC {
    method_321: ResourceReference
});

codec_struct!(BattleInfoUser {
    kills: i32
    score: i32
    suspicious: bool
    user: Option<String>
});

codec_struct!(BattleInfoData {
    battle_id: Option<String>
    map_name: Option<String>
    mode: BattleMode
    private_battle: bool
    pro_battle: bool
    range: Range
    server_number: i32
});

codec_struct!(BattleCreateParameters {
    auto_balance: bool
    battle_mode: BattleMode
    equipment_constraints_mode: EquipmentConstraintsMode
    friendly_fire: bool
    method_1309: BattleLimits
    map_id: Option<String>
    max_people_count: i32
    name: Option<String>
    parkour_mode: bool
    private_battle: bool
    pro_battle: bool
    rank_range: Range
    re_armor_enabled: bool
    theme: MapTheme
    without_bonuses: bool
    without_crystals: bool
    without_supplies: bool
});

codec_struct!(AssaultSoundFX {
    name_55: ResourceReference
    name_79: ResourceReference
    name_37: ResourceReference
    name_71: ResourceReference
});

codec_struct!(AssaultCC {
    method_874: Vec<ClientAssaultFlag>
    method_2535: ResourceReference
    method_1333: ResourceReference
    method_1036: ResourceReference
    method_2134: ResourceReference
    method_993: Vec<AssaultBase>
    name_8: AssaultSoundFX
});

codec_struct!(AssaultBase {
    id: i32
    position: Option<Vector3<f32>>
});

codec_struct!(AchievementCC {
    method_2426: Vec<Achievement>
});

codec_struct!(UidNotifierData {
    uid: String
    userId: String
});