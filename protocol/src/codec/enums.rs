use crate::codec::Codeable;
use crate::ProtocolResult;
use crate::ProtocolError;
use std::any::type_name;

macro_rules! codec_enum {
    ($name:ident : $type:ty { $($variant:ident = $value:expr)* }) => {
        #[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
        pub enum $name {
            Unknown = -1,

            $($variant = $value),*
        }

        impl Default for $name {
            fn default() -> Self {
                $name::Unknown
            }
        }

        impl Codeable for $name {
            fn encode(&self, writer: &mut dyn std::io::Write) -> ProtocolResult<()> {
                let code: $type = match self {
                    $($name::$variant => $value,)*
                    $name::Unknown => return Err(ProtocolError::EnumUnknown), 

                };
                code.encode(writer)
            }

            fn decode(&mut self, reader: &mut dyn std::io::Read) -> ProtocolResult<()> {
                let mut code = <$type>::default();
                code.decode(reader)?;

                *self = match code {
                    $($value => $name::$variant,)*
                    value => return Err(
                        ProtocolError::EnumInvalidOrdinal(value as u64, type_name::<$name>().to_string())
                    )
                };
                Ok(())
            }
        }
    };
}

codec_enum!(CaptchaLocation : i32 {
    LoginForm = 0
    RegisterForm = 1
    ClientStartup = 2
    RestorePasswordForm = 3
    EmailChangeHash = 4
    AccountSettingsForm = 5
});

codec_enum!(LayoutState : i32 {
    BattleSelect = 0
    Garage = 1
    Payment = 2
    Battle = 3
    ReloadSpace = 4
});

codec_enum!(ValidationStatus : i32 {
    TooShort = 0
    TooLong = 1
    NotUnique = 2
    NotMatchPattern = 3
    Forbidden = 4
    Correct = 5
});

codec_enum!(MapTheme : i32 {
    Summer = 0
    Winter = 1
    Space = 2
    SummerDay = 3
    SummerNight = 4
    WinterDay = 5
});

codec_enum!(ItemViewCategory : i32 {
    Weapon = 0
    Armor = 1
    Paint = 2
    Inventory = 3
    Kit = 4
    Special = 5
    GivenPresents = 6
});

codec_enum!(ItemCategory : i32 {
    Weapon = 0
    Armor = 1
    Color = 2
    Inventory = 3
    Plugin = 4
    Kit = 5
    Emblem = 6
    Present = 7
    GivenPresent = 8
});

codec_enum!(IsisState : i32 {
    Off = 0
    Idle = 1
    Healing = 2
    Damaging = 3
});

codec_enum!(EquipmentConstraintsMode : i32 {
    None = 0
    HornetRailgun = 1
    WaspRailgun = 2
    HornetWaspRailgun = 3
});

codec_enum!(DamageIndicatorType : i32 {
    Normal = 0
    Critical = 1
    Fatal = 2
    Heal = 3
});

codec_enum!(ControlPointState : i32 {
    Red = 0
    Blue = 1
    Neutral = 2
});

codec_enum!(ChatModeratorLevel : i32 {
    None = 0
    CommunityManager = 1
    Administrator = 2
    Moderator = 3
    Candidate = 4
});

codec_enum!(BattleTeam : i32 {
    Red = 0
    Blue = 1
    None = 2
});

codec_enum!(BattleSuspicionLevel : i32 {
    None = 0
    Low = 1
    High = 2
});

codec_enum!(BattleMode : i32 {
    Dm = 0
    Tdm = 1
    Ctf = 2
    Cp = 3
    As = 4
});

codec_enum!(Achievement : i32 {
    FirstRankUp = 0
    FirstPurchase = 1
    SetEmail = 2
    FightFirstBattle = 3
    FirstDonate = 4
});