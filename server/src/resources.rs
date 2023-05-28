use std::{path::Path, fs::File, collections::BTreeMap, sync::Arc};

use anyhow::Context;

use crate::server::Server;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum ResourceStage {
    Auth,
    Connect,
    Lobby,
}

type ResourceId = u64;
pub struct ServerResources {
    resource_stage: BTreeMap<ResourceStage, Vec<ResourceId>>,
    resources: BTreeMap<ResourceId, Arc<ServerResource>>,
}

macro_rules! load_resources_file {
    ($filename:literal) => {{
        let resources = serde_json::from_str::<Vec<json::Resource>>(
            include_str!($filename)
        )?;

        resources.into_iter()
            .map(|res| ServerResource::from_json_resource(res))
            .try_collect::<Vec<_>>()?
            .into_iter()
            .map(|res| Arc::new(res))
            .collect::<Vec<_>>()
    }};
}

impl ServerResources {
    pub fn new() -> anyhow::Result<Self> {
        let mut result = Self {
            resource_stage: Default::default(),
            resources: Default::default()
        };
        
        result.register_resources_for_stage(
            ResourceStage::Connect,
            load_resources_file!("../resources/registry/connect.json") 
        );

        result.register_resources_for_stage(
            ResourceStage::Auth,
            load_resources_file!("../resources/registry/auth.json") 
        );
        
        result.register_resources_for_stage(
            ResourceStage::Lobby,
            load_resources_file!("../resources/registry/lobby.json") 
        );

        Ok(result)
    }

    fn register_resources_for_stage(&mut self, stage: ResourceStage, resources: Vec<Arc<ServerResource>>) {
        for resource in resources.iter().cloned() {
            self.resources.insert(resource.id, resource);
        }

        self.resource_stage.insert(
            stage, 
            resources.iter()
                .map(|resource| resource.id)
                .collect::<Vec<_>>()
        );
    }

    pub fn get_resources(&self, stage: ResourceStage) -> Vec<Arc<ServerResource>> {
        let resource_ids = match self.resource_stage.get(&stage) {
            Some(resource_ids) => resource_ids,
            None => return vec![],
        };

        resource_ids.iter()
            .filter_map(|res_id| self.resources.get(&res_id))
            .cloned()
            .collect::<Vec<_>>()
    }
}

pub struct ServerResource {
    pub id: ResourceId,
    pub version: u64,
    pub lazy: bool,
    pub info: ResourceInfo,
}

#[derive(Debug)]
pub enum ResourceInfo {
    SwfLibrary,
    A3D,
    MovieClip,
    Sound,
    Model3DS,

    Map,
    PropLib,

    Image { alpha: bool },
    MultiframeImage { fps: u32, height: u32, width: u32, frames: u32 },
    LocalizedImage {  file_names: Vec<String>, alpha: bool },

    Tanks3DS
    // this.resourceRegistry.registerTypeClasses(MapResource.TYPE,MapResource);
    // this.resourceRegistry.registerTypeClasses(PropLibResource.TYPE,PropLibResource);
    // this.resourceRegistry.registerTypeClasses(Tanks3DSResource.TYPE,Tanks3DSResource);
}

impl ServerResource {
    pub fn from_json_resource(resource: json::Resource) -> anyhow::Result<Self> {
        let info = match resource.resource_type {
            1 => ResourceInfo::SwfLibrary,
            2 => ResourceInfo::A3D,
            3 => ResourceInfo::MovieClip,
            4 => ResourceInfo::Sound,
            
            7 => ResourceInfo::Map,
            8 => ResourceInfo::PropLib,

            9 => ResourceInfo::Model3DS,
            10 => ResourceInfo::Image {
                alpha: resource.alpha.context("missing alpha flag")?
            },
            11 => ResourceInfo::MultiframeImage { 
                fps: resource.fps.context("missing fps")?,
                height: resource.height.context("missing height")?,
                width: resource.weight.context("missing width")?,
                frames: resource.num_frames.context("missing frames")?
            },
            13 => ResourceInfo::LocalizedImage { 
                file_names: resource.file_names.context("missing filenames")?, 
                alpha: resource.alpha.context("missing alpha flag")?
            },
            17 => ResourceInfo::Tanks3DS,
            _ => anyhow::bail!("unknown resource type {}", resource.resource_type)
        };

        Ok(Self {
            id: (resource.idhigh.parse::<u64>()? << 32) | (resource.idlow as u64),
            version: (resource.versionhigh.parse::<u64>()? << 32) | (resource.versionlow as u64),
            lazy: resource.lazy,
            info
        })
    }

    pub fn as_json_resource(&self) -> json::Resource {
        let mut response = json::Resource::default();

        response.idhigh = format!("{}", self.id >> 32);
        response.idlow = (self.id & 0xFFFFFFFF) as u32;

        response.versionhigh = format!("{}", self.version >> 32);
        response.versionlow = (self.version & 0xFFFFFFFF) as u32;

        response.lazy = self.lazy;
        match &self.info {
            ResourceInfo::SwfLibrary => {
                response.resource_type = 1;
            },
            ResourceInfo::A3D => {
                response.resource_type = 2;
            },
            ResourceInfo::MovieClip => {
                response.resource_type = 3;
            },
            ResourceInfo::Sound => {
                response.resource_type = 4;
            },
            ResourceInfo::Map => {
                response.resource_type = 7;
            },
            ResourceInfo::PropLib => {
                response.resource_type = 8;
            },
            ResourceInfo::Model3DS => {
                response.resource_type = 9;
            },
            ResourceInfo::Image { alpha } => {
                response.resource_type = 10;
                response.alpha = Some(*alpha);
            },
            ResourceInfo::MultiframeImage { fps, height, width, frames } => {
                response.resource_type = 11;
                response.fps = Some(*fps);
                response.height = Some(*height);
                response.weight = Some(*width);
                response.num_frames = Some(*frames);
            },
            ResourceInfo::LocalizedImage { alpha, file_names } => {
                response.resource_type = 13;
                response.alpha = Some(*alpha);
                response.file_names = Some(file_names.clone());
            },
            ResourceInfo::Tanks3DS => {
                response.resource_type = 17;
            }
        }
        response
    }
}

pub mod json {
    use serde::{Serialize, Deserialize};

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Resource {
        pub idhigh: String,
        pub idlow: u32,

        pub versionhigh: String,
        pub versionlow: u32,

        #[serde(rename = "type")]
        pub resource_type: i64,
        pub lazy: bool,

        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(default)]
        pub alpha: Option<bool>,
        
        
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(default)]
        pub weight: Option<u32>,
        
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(default)]
        pub height: Option<u32>,
        
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(default)]
        pub num_frames: Option<u32>,
        
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(default)]
        pub fps: Option<u32>,
        
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(default)]
        pub file_names: Option<Vec<String>>,
    }
}