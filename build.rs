use std::{env, path::Path, fs::File, io::{BufReader, Write, self}, collections::BTreeMap};
use convert_case::{Casing, Case};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct PacketDescription {
    name: Option<String>,

    packet_id: i32,
    model_id: u32,

    codecs: Vec<String>,
    fields: Vec<String>,
}

const TEMPLATE_FOOTER: &'static str = r#"
pub fn register_all_packets(registry: &mut PacketRegistry) {
#impl_register#
}
"#;

const TEMPLATE_PACKET: &'static str = r#"
#[derive(Default, Clone, Debug)]
pub struct #name# {
#fields#
}

impl Packet for #name# {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn packet_name(&self) -> &str {
        type_name::<Self>()
    }

    fn packet_id(&self) -> u32 {
        #packet_id#
    }

    fn model_id(&self) -> u32 {
        #module_id#
    }
}

#[allow(unused_mut)]
#[allow(unused_variables)]
impl Codec for #name# {
    type Target = Self;

    fn encode(self: &Self, registry: &CodecRegistry, writer: &mut dyn Write, target: &Self::Target) -> anyhow::Result<()> {
#encode#
        return Ok(());
    }

    fn decode(self: &Self, registry: &CodecRegistry, reader: &mut dyn Read) -> anyhow::Result<Self::Target> {
        let mut result: Self::Target = Default::default();
#decode#
        return Ok(result);
    }
}
"#;

fn generate_packet_class(writer: &mut dyn Write, description: &PacketDescription, codecs: &BTreeMap<String, String>) -> io::Result<bool> {
    let name = match &description.name {
        Some(value) => value,
        None => {
            writeln!(writer, "/* Skipping {} (Model {}) because we haven't yet assigned a name */", description.packet_id, description.model_id)?;
            return Ok(false);
        }
    };

    let mut rust_codecs = Vec::with_capacity(codecs.len());
    for codec in description.codecs.iter() {
        let rust_codec = match codecs.get(codec) {
            Some(value) => value,
            None => {
                writeln!(writer, "/* Skipping {} (Model {}) because we haven't yet implemented the codec {} */", description.packet_id, description.model_id, codec)?;
                return Ok(false);
            }
        };

        rust_codecs.push(rust_codec);
    }

    let rust_field_names = description.fields.iter()
        .map(|name| name.to_case(Case::Snake))
        .collect::<Vec<_>>();

    let fields = rust_field_names.iter().zip(rust_codecs.iter())
        .map(|(field_name, field_type)| format!("    pub {}: {},", field_name, field_type))
        .collect::<Vec<_>>()
        .join("\n");

    let encode = rust_field_names.iter()
        .map(|field_name| {
            format!("        registry.encode(writer, &target.{})?;", field_name)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let decode = rust_field_names.iter()
        .map(|field_name| {
            format!("        result.{} = registry.decode(reader)?;", field_name)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let class_data = TEMPLATE_PACKET
        .replace("#name#", &name)
        .replace("#packet_id#", &format!("({}i32) as u32", description.packet_id))
        .replace("#module_id#", &format!("{}", description.model_id))
        .replace("#fields#", &fields)

        .replace("#encode#", &encode)
        .replace("#decode#", &decode);

    write!(writer, "{}", class_data)?;
    Ok(true)
}

fn generate_footer(writer: &mut dyn Write, packets: &[&PacketDescription]) -> io::Result<()> {
    let impl_register = packets.iter()
        .map(|packet| format!("    registry.register_packet::<{}>(Default::default());", packet.name.as_ref().expect("a packet class name")))
        .collect::<Vec<_>>()
        .join("\n");

    let class_data = TEMPLATE_FOOTER
        .replace("#impl_register#", &impl_register);

    write!(writer, "{}", class_data)?;
    Ok(())
}

fn main() {
	let out_dir = env::var("OUT_DIR").unwrap();
    println!("Out dir: {}", out_dir);

	let path = Path::new(&out_dir);
	let mut packets = File::create(&path.join("packets.rs")).unwrap();

    let packet_schema = File::open("resources/pt_packet_schema.json").unwrap();
    let packet_schema = BufReader::new(packet_schema);
    let packet_schema: Vec<PacketDescription> = serde_json::from_reader(packet_schema).unwrap();

    let codec_mapping = File::open("resources/codec_mapping.json").unwrap();
    let codec_mapping = BufReader::new(codec_mapping);
    let codec_mapping: BTreeMap<String, String> = serde_json::from_reader(codec_mapping).unwrap();

    /* TODO: Write header */
    writeln!(&mut packets, "use crate::codec::*;").unwrap();
    let mut generated_packets = Vec::with_capacity(packet_schema.len());
    for packet in packet_schema.iter() {
        let generated = generate_packet_class(&mut packets, packet, &codec_mapping).unwrap();
        if !generated {
            continue;
        }

        generated_packets.push(packet);
    }
    generate_footer(&mut packets, &generated_packets).unwrap();
}