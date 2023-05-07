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