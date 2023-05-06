use std::{any::{Any, TypeId, type_name}, io::{Write, Read}, sync::Arc, collections::BTreeMap};
use anyhow::anyhow;

mod primitives;
pub use primitives::*;

mod custom;
pub use custom::*;

pub trait Codec : Send + Sync {
    type Target: Sized + Any;

    fn encode(&self, registry: &CodecRegistry, writer: &mut dyn Write, target: &Self::Target) -> anyhow::Result<()>;

    fn decode(&self, registry: &CodecRegistry, reader: &mut dyn Read) -> anyhow::Result<Self::Target>;
}

struct RegisteredCodec<T> {
    codec: Arc<dyn Codec<Target = T>>
}

impl<T: 'static> RegisteredCodec<T> {
    pub fn new(codec: impl Codec<Target = T> + 'static + Sized) -> Self {
        Self {
            codec: Arc::new(codec)
        }
    }
}

#[derive(Default)]
pub struct CodecRegistry {
    codecs: BTreeMap<TypeId, Box<dyn Any + Send>>
}

impl CodecRegistry {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn register_primatives(&mut self) {
        self.register_codec(BoolCodec{});
        self.register_codec(ByteCodec{});
        self.register_codec(UByteCodec{});
        self.register_codec(ShortCodec{});
        self.register_codec(UShortCodec{});
        self.register_codec(IntCodec{});
        self.register_codec(UIntCodec{});
        self.register_codec(LongCodec{});
        self.register_codec(ULongCodec{});
        self.register_codec(FloatCodec{});
        self.register_codec(DoubleCodec{});
        self.register_codec(LengthCodec{});
        self.register_codec(StringCodec{});
        
        self.register_codec(VectorCodec::<i8>::default());
        self.register_codec(VectorCodec::<String>::default());
    }

    pub fn register_codec<T: 'static>(&mut self, codec: impl Codec<Target = T> + 'static + Sized) -> Arc<dyn Codec<Target = T>> {
        let codec = RegisteredCodec::new(codec);
        let result = codec.codec.clone();
        self.codecs.insert(TypeId::of::<T>(), Box::new(codec));
        result
    }

    #[must_use]
    pub fn get_codec<T: 'static>(&self) -> Option<Arc<dyn Codec<Target = T>>> {
        self.codecs.get(&TypeId::of::<T>())
            .map(|r| r.downcast_ref::<RegisteredCodec<T>>())
            .flatten()
            .map(|r| r.codec.clone())
    }

    #[must_use]
    pub fn decode<T: 'static>(&self, reader: &mut dyn Read) -> anyhow::Result<T> {
        self.get_codec::<T>()
                .ok_or_else(|| anyhow!("no codec registered for {}", type_name::<T>()))?
                .decode(self, reader)
    }

    #[must_use]
    pub fn encode<T: 'static>(&self, writer: &mut dyn Write, target: &T) -> anyhow::Result<()> {
        self.get_codec::<T>()
                .ok_or_else(|| anyhow!("no codec registered for {}", type_name::<T>()))?
                .encode(self, writer, target)
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::CodecRegistry;

    #[test]
    fn basic_test() {
        let mut codecs = CodecRegistry::new();
        codecs.register_primatives();
    
        let mut buffer = Vec::with_capacity(16);
        codecs.encode(&mut Cursor::new(&mut buffer), &"Hello World!".to_owned()).unwrap();
        let decoded: String = codecs.decode(&mut Cursor::new(buffer.as_slice())).unwrap();

        assert_eq!(decoded, "Hello World!");
    }
}