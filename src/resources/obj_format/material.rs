use {
    super::super::texture::Texture,
    glam::Vec3,
    std::{path::PathBuf, sync::Arc},
    tobj::Material as tobjMaterial,
    windows::Win32::Graphics::Direct3D11::ID3D11Device,
};

#[allow(unused)]
pub struct ColouredMaterialTexture {
    pub texture: Arc<Texture>,
    pub colour: Vec3,
}
#[allow(unused)]
pub struct AttributedMaterialTexture {
    pub texture: Arc<Texture>,
    pub attribute: f32,
}

#[allow(unused)]
#[derive(Default)]
pub struct ObjMaterial {
    pub ambient: Option<ColouredMaterialTexture>,
    pub diffuse: Option<ColouredMaterialTexture>,
    pub specular: Option<ColouredMaterialTexture>,
    pub normal: Option<Arc<Texture>>,
    pub shininess: Option<AttributedMaterialTexture>,
    pub dissolve: Option<AttributedMaterialTexture>,
}

pub struct ObjMaterials {
    pub materials: Vec<tobjMaterial>,
    pub folder: PathBuf,
}

impl ObjMaterials {
    pub fn load(&self, device: &ID3D11Device, idx: usize) -> anyhow::Result<ObjMaterial> {
        let material = &self.materials[idx];
        let device_context = unsafe { device.GetImmediateContext() }.expect("I lost my context!");

        let ambient =
            if let (Some(texture), Some(value)) = (&material.ambient_texture, &material.ambient) {
                let texture_path = self.folder.join(PathBuf::from(&texture));
                let texture = Texture::load(device, &texture_path)?;
                let colour = Vec3::from_slice(value);
                Some(ColouredMaterialTexture { texture, colour })
            } else {
                None
            };
        let diffuse =
            if let (Some(texture), Some(value)) = (&material.diffuse_texture, &material.diffuse) {
                let texture_path = self.folder.join(PathBuf::from(&texture));
                let texture = Texture::load(device, &texture_path)?;
                texture.generate_mips(&device_context);
                let colour = Vec3::from_slice(value);
                Some(ColouredMaterialTexture { texture, colour })
            } else {
                None
            };
        let specular = if let (Some(texture), Some(value)) =
            (&material.specular_texture, &material.specular)
        {
            let texture_path = self.folder.join(PathBuf::from(&texture));
            let texture = Texture::load(device, &texture_path)?;
            let colour = Vec3::from_slice(value);
            Some(ColouredMaterialTexture { texture, colour })
        } else {
            None
        };
        let normal = if let Some(texture) = &material.normal_texture {
            let texture_path = self.folder.join(PathBuf::from(&texture));
            Some(Texture::load(device, &texture_path)?)
        } else {
            None
        };
        let shininess = if let (Some(texture), Some(attribute)) =
            (&material.shininess_texture, material.shininess)
        {
            let texture_path = self.folder.join(PathBuf::from(&texture));
            let texture = Texture::load(device, &texture_path)?;
            Some(AttributedMaterialTexture { texture, attribute })
        } else {
            None
        };
        let dissolve = if let (Some(texture), Some(attribute)) =
            (&material.dissolve_texture, material.dissolve)
        {
            let texture_path = self.folder.join(PathBuf::from(&texture));
            let texture = Texture::load(device, &texture_path)?;
            Some(AttributedMaterialTexture { texture, attribute })
        } else {
            None
        };
        let material_set = ObjMaterial {
            ambient,
            diffuse,
            specular,
            normal,
            shininess,
            dissolve,
        };

        Ok(material_set)
    }
}
