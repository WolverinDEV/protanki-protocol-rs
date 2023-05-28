use std::io::Cursor;

use byteorder::{ReadBytesExt, BigEndian};

pub enum ResourceType {
    SwfLibrary = 1,
    A3D = 2,
    MovieClip = 3,
    Sound = 4,
    Model3DS = 9,
    Image = 10,
    MultiframeImage = 11,
    LocalizedImage = 13,
}

pub fn build_resource_path(resource_id: u64, version: u64) -> String {
    let buffer = resource_id.to_be_bytes();
    let mut cursor = Cursor::new(&buffer);

    format!(
        "{}/{}/{}/{}/{:o}",
        cursor.read_u32::<BigEndian>().unwrap_or(0),
        cursor.read_u16::<BigEndian>().unwrap_or(0),
        cursor.read_u8().unwrap_or(0),
        cursor.read_u8().unwrap_or(0),
        version
    )
}

#[cfg(test)]
mod test {
    use crate::resources::build_resource_path;

    #[test]
    fn resource_ids() {
        assert_eq!(
            build_resource_path(1395316, 1),
            "0/21/74/116/1"
        );
        
        assert_eq!(
            build_resource_path(1395316, 10),
            "0/21/74/116/12"
        );
    }
}