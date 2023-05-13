use nalgebra::Vector3;

use super::Codec;

pub type ResourceId = u32;

pub struct ResourceIdCodec;
impl Codec for ResourceIdCodec {
    type Target = ResourceId;

    fn encode(self: &Self, registry: &super::CodecRegistry, writer: &mut dyn std::io::Write, target: &Self::Target) -> anyhow::Result<()> {
        registry.encode::<u32>(writer, target)
    }

    fn decode(self: &Self, registry: &super::CodecRegistry, reader: &mut dyn std::io::Read) -> anyhow::Result<Self::Target> {
        registry.decode::<u32>(reader)
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
pub enum CaptchaLocation {
    LoginForm,
    RegisterForm,
    ClientStartup,
    RestorePasswordForm,
    EmailChangeHash,
    AccountSettingsForm,
}

impl Default for CaptchaLocation {
    fn default() -> Self {
        CaptchaLocation::LoginForm
    }
}

pub struct CaptchaLocationCodec;
impl Codec for CaptchaLocationCodec {
    type Target = CaptchaLocation;

    fn encode(self: &Self, registry: &super::CodecRegistry, writer: &mut dyn std::io::Write, target: &Self::Target) -> anyhow::Result<()> {
        let code = match target {
            CaptchaLocation::LoginForm => 0,
            CaptchaLocation::RegisterForm => 1,
            CaptchaLocation::ClientStartup => 2,
            CaptchaLocation::RestorePasswordForm => 3,
            CaptchaLocation::EmailChangeHash => 4,
            CaptchaLocation::AccountSettingsForm => 5,
        };
        registry.encode::<i32>(writer, &code)
    }

    fn decode(self: &Self, registry: &super::CodecRegistry, reader: &mut dyn std::io::Read) -> anyhow::Result<Self::Target> {
        Ok(
            match registry.decode::<i32>(reader)? {
                0 => CaptchaLocation::LoginForm,
                1 => CaptchaLocation::RegisterForm,
                2 => CaptchaLocation::ClientStartup,
                3 => CaptchaLocation::RestorePasswordForm,
                4 => CaptchaLocation::EmailChangeHash,
                5 => CaptchaLocation::AccountSettingsForm,
                _ => anyhow::bail!("invalid captcha location code")
            }
        )
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
pub enum BattleTeam {
    None,
    Red,
    Blue
}
impl Default for BattleTeam {
    fn default() -> Self {
        BattleTeam::None
    }
}

pub struct CodecBattleTeam;
impl Codec for CodecBattleTeam {
    type Target = BattleTeam;

    fn encode(&self, registry: &super::CodecRegistry, writer: &mut dyn std::io::Write, target: &Self::Target) -> anyhow::Result<()> {
        let code = match target {
            BattleTeam::Red => 0,
            BattleTeam::Blue => 1,
            BattleTeam::None => 2,
        };
        registry.encode::<i32>(writer, &code)
    }

    fn decode(&self, registry: &super::CodecRegistry, reader: &mut dyn std::io::Read) -> anyhow::Result<Self::Target> {
        Ok(
            match registry.decode::<i32>(reader)? {
                0 => BattleTeam::Red,
                1 => BattleTeam::Blue,
                2 => BattleTeam::None,
                _ => anyhow::bail!("invalid battle team")
            }
        )
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
pub enum LayoutState {
    BattleSelect,
    Garage,
    Payment,
    Battle,
    ReloadSpace
}
impl Default for LayoutState {
    fn default() -> Self {
        LayoutState::BattleSelect
    }
}

pub struct CodecLayoutState;
impl Codec for CodecLayoutState {
    type Target = LayoutState;

    fn encode(&self, registry: &super::CodecRegistry, writer: &mut dyn std::io::Write, target: &Self::Target) -> anyhow::Result<()> {
        let code = match target {
            LayoutState::BattleSelect => 0,
            LayoutState::Garage => 1,
            LayoutState::Payment => 2,
            LayoutState::Battle => 3,
            LayoutState::ReloadSpace => 4,
        };
        registry.encode::<i32>(writer, &code)
    }

    fn decode(&self, registry: &super::CodecRegistry, reader: &mut dyn std::io::Read) -> anyhow::Result<Self::Target> {
        Ok(
            match registry.decode::<i32>(reader)? {
                0 => LayoutState::BattleSelect,
                1 => LayoutState::Garage,
                2 => LayoutState::Payment,
                3 => LayoutState::Battle,
                4 => LayoutState::ReloadSpace,
                _ => anyhow::bail!("invalid layout state")
            }
        )
    }
}

pub struct CodecVector3d;
impl Codec for CodecVector3d {
    type Target = Vector3<f32>;

    fn encode(&self, registry: &super::CodecRegistry, writer: &mut dyn std::io::Write, target: &Self::Target) -> anyhow::Result<()> {
        registry.encode::<f32>(writer, &target.x)?;
        registry.encode::<f32>(writer, &target.y)?;
        registry.encode::<f32>(writer, &target.z)?;
        Ok(())
    }

    fn decode(&self, registry: &super::CodecRegistry, reader: &mut dyn std::io::Read) -> anyhow::Result<Self::Target> {
        let x = registry.decode::<f32>(reader)?;
        let y = registry.decode::<f32>(reader)?;
        let z = registry.decode::<f32>(reader)?;
        Ok(Vector3::new(x, y, z))
    }
}

#[derive(Debug, Default, Clone)]
pub struct RotateTurretCommand {
    pub target: f32,
    pub control: u8,
}

pub struct CodecRotateTurretCommand;
impl Codec for CodecRotateTurretCommand {
    type Target = RotateTurretCommand;

    fn encode(&self, registry: &super::CodecRegistry, writer: &mut dyn std::io::Write, target: &Self::Target) -> anyhow::Result<()> {
        registry.encode::<f32>(writer, &target.target)?;
        registry.encode::<u8>(writer, &target.control)
    }

    fn decode(&self, registry: &super::CodecRegistry, reader: &mut dyn std::io::Read) -> anyhow::Result<Self::Target> {
        let mut result = RotateTurretCommand::default();
        result.target = registry.decode::<f32>(reader)?;
        result.control = registry.decode::<u8>(reader)?;
        Ok(result)
    }
}

#[derive(Debug, Default, Clone)]
pub struct MoveCommand {
    pub control: u8,
    pub position: Option<Vector3<f32>>,
    pub orientation: Option<Vector3<f32>>,
    pub velocity: Option<Vector3<f32>>,
    pub angular_velocity: Option<Vector3<f32>>,
}

pub struct CodecMoveCommand;
impl Codec for CodecMoveCommand {
    type Target = MoveCommand;

    fn encode(&self, registry: &super::CodecRegistry, writer: &mut dyn std::io::Write, target: &Self::Target) -> anyhow::Result<()> {
        registry.encode(writer, &target.angular_velocity)?;
        registry.encode(writer, &target.control)?;
        registry.encode(writer, &target.velocity)?;
        registry.encode(writer, &target.orientation)?;
        registry.encode(writer, &target.position)?;
        Ok(())
    }

    fn decode(&self, registry: &super::CodecRegistry, reader: &mut dyn std::io::Read) -> anyhow::Result<Self::Target> {
        let mut result = MoveCommand::default();
        result.angular_velocity = registry.decode(reader)?;
        result.control = registry.decode(reader)?;
        result.velocity = registry.decode(reader)?;
        result.orientation = registry.decode(reader)?;
        result.position = registry.decode(reader)?;
        Ok(result)
    }
}