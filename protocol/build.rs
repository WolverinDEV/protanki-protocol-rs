use std::{env, path::Path, fs::File, io::{BufReader, Write, self}, collections::BTreeMap};
use convert_case::{Casing, Case};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PacketDirection {
    S2C,
    C2S,
    X2X
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug)]
struct PacketDescription {
    direction: PacketDirection,
    
    packet_id: i32,
    model_id: u32,

    #[serde_as(as = "BTreeMap<_, _>")]
    #[serde(default)]
    fields: Vec<(String, String)>,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug)]
struct ModelDescription {
    model_id: u32,
    
    #[serde_as(as = "BTreeMap<_, _>")]
    #[serde(default)]
    packets: Vec<(String, PacketDescription)>,
}

const TEMPLATE_FILE_HEADER: &'static str = r#"
/// *** ATTENTION: This file has been automatically generated. DO NOT MODIFY! ***
/// Generated packet struct given the packets.yml definition file.
"#;

const TEMPLATE_MODULE_HEADER: &'static str = r#"
use std::any::{ type_name, Any };
use std::io::{ Read, Write };
use nalgebra::Vector3;
use crate::ProtocolResult;
use crate::packets::*;
use crate::codec::*;
"#;

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

    fn direction(&self) -> PacketDirection {
        PacketDirection::#direction#
    }

    fn packet_id(&self) -> u32 {
        #packet_id#
    }

    fn model_id(&self) -> u32 {
        #module_id#
    }

    fn encode(&self, writer: &mut dyn Write) -> ProtocolResult<()> {
#encode#
        return Ok(());
    }

    fn decode(&mut self, reader: &mut dyn Read) -> ProtocolResult<()> {
#decode#
        return Ok(());
    }
}
"#;

fn generate_packet_class(
    writer: &mut dyn Write, 
    name: &str,
    direction: PacketDirection,
    description: &PacketDescription, 
    codecs: &BTreeMap<String, String>,
    packet_names: &mut Vec<String>,
) -> anyhow::Result<()> {
    let mut rust_codecs = Vec::with_capacity(codecs.len());
    for (_, codec) in description.fields.iter() {
        let rust_codec = match codecs.get(codec) {
            Some(value) => value,
            None => {
                panic!("packet {} (Model {}) contains unknown codec {}", description.packet_id, description.model_id, codec);
            }
        };

        rust_codecs.push(rust_codec);
    }

    let rust_field_names = description.fields.iter()
        .map(|(name, _): &(String, String)| name.to_case(Case::Snake))
        .collect::<Vec<_>>();

    let fields = rust_field_names.iter().zip(rust_codecs.iter())
        .map(|(field_name, field_type)| format!("    pub {}: {},", field_name, field_type))
        .collect::<Vec<_>>()
        .join("\n");

    let encode = rust_field_names.iter()
        .map(|field_name| {
            format!("        self.{}.encode(writer)?;", field_name)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let decode = rust_field_names.iter()
        .map(|field_name| {
            format!("        self.{}.decode(reader)?;", field_name)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let direction_name = match direction {
        PacketDirection::C2S => "C2S",
        PacketDirection::S2C => "S2C",
        _ => anyhow::bail!("expected C2S or S2C packet direction"),
    };

    let class_data = TEMPLATE_PACKET
        .replace("#name#", &name)
        .replace("#packet_id#", &format!("({}i32) as u32", description.packet_id))
        .replace("#module_id#", &format!("{}", description.model_id))
        .replace("#fields#", &fields)

        .replace("#direction#", direction_name)
        .replace("#encode#", &encode)
        .replace("#decode#", &decode);

    write!(writer, "{}", class_data)?;
    packet_names.push(name.to_string());

    Ok(())
}

fn generate_footer(writer: &mut dyn Write, packets: &[String]) -> io::Result<()> {
    let impl_register = packets.iter()
        .map(|name| format!("    registry.register_packet::<{}>();", name))
        .collect::<Vec<_>>()
        .join("\n");

    let class_data = TEMPLATE_FOOTER
        .replace("#impl_register#", &impl_register);

    write!(writer, "{}", class_data)?;
    Ok(())
}

macro_rules! load_file {
    ($name:literal, $parser:expr) => {
        {
            let payload = File::open($name)?;
            let payload = BufReader::new(payload);
            $parser(payload)?
        }
    }
}

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=resources/codec_mapping.json");
    println!("cargo:rerun-if-changed=resources/packets.yml");

	let out_dir = env::var("OUT_DIR")?;

	let path = Path::new(&out_dir);
	let mut packets = File::create(&path.join("packets.rs"))?;

    let packet_schema: BTreeMap<String, ModelDescription> = load_file!("resources/packets.yml", serde_yaml::from_reader);
    let codec_mapping: BTreeMap<String, String> = load_file!("resources/codec_mapping.json", serde_json::from_reader);

    let flat_packets = packet_schema.iter()
        .map(|(m, v)| {
            v.packets.iter()
                .map(move |(p, v)| (format!("{}{}", m, p), v))
        })
        .flatten()
        .collect::<Vec<_>>();

    write!(&mut packets, "{}", TEMPLATE_FILE_HEADER)?;
    for (dir_name, direction) in [
        ("c2s", PacketDirection::C2S),
        ("s2c", PacketDirection::S2C)
    ] {
        writeln!(&mut packets, "pub mod {} {{", dir_name)?;
        write!(&mut packets, "{}", TEMPLATE_MODULE_HEADER)?;
        let mut generated_packets = Vec::with_capacity(packet_schema.len());
        for (name, packet) in flat_packets.iter() {
            if packet.direction != direction && packet.direction != PacketDirection::X2X {
                continue;
            }

            generate_packet_class(&mut packets, name, direction, packet, &codec_mapping, &mut generated_packets)?;
        }
        generate_footer(&mut packets, &generated_packets)?;
        writeln!(&mut packets, "}}")?;
    }
    Ok(())
}