use naga_oil::compose::{
    ComposableModuleDescriptor, Composer, NagaModuleDescriptor, ShaderDefValue,
};
use std::borrow::Cow;
use std::collections::HashMap;
use wgpu;

struct ShaderMaker {
    composer: Composer,
}

impl ShaderMaker {
    pub fn new() -> Self {
        let composer = Composer::default();

        Self { composer }
    }

    pub fn load(
        &mut self,
        source: &str,
        all_shader_defs: &[&str],
        defined_shader_defs: &[&str],
    ) -> Option<wgpu::ShaderSource> {
        let mut load_composable =
            |source: &str, shader_defs: HashMap<String, ShaderDefValue>| match self
                .composer
                .add_composable_module(ComposableModuleDescriptor {
                    source,
                    shader_defs,
                    ..Default::default()
                }) {
                Ok(module) => {
                    println!(
                        "Added composable module {} [{:?}]",
                        module.name, module.shader_defs
                    )
                }
                Err(e) => {
                    println!("? -> {e:#?}")
                }
            };

        let mut all_shader_defs_map: HashMap<String, ShaderDefValue> = HashMap::new();
        for def in all_shader_defs.iter() {
            all_shader_defs_map.insert(def.to_owned().parse().unwrap(), Default::default());
        }
        load_composable(source, all_shader_defs_map.into());

        let mut defined_shader_defs_map: HashMap<String, ShaderDefValue> = HashMap::new();
        for def in defined_shader_defs.iter() {
            defined_shader_defs_map.insert(def.to_owned().parse().unwrap(), Default::default());
        }

        match self.composer.make_naga_module(NagaModuleDescriptor {
            source: include_str!("model.wgsl"),
            shader_defs: defined_shader_defs_map.into(),
            ..Default::default()
        }) {
            Ok(module) => Some(wgpu::ShaderSource::Naga(Cow::Owned(module))),
            Err(e) => {
                println!("{}", e.emit_to_string(&self.composer));
                None
            }
        }
    }
}

fn main() {
    let mut shader_maker = ShaderMaker::new();

    let shader_source = shader_maker.load(
        include_str!("model.wgsl"),
        &["COLOR_MAP", "NORMAL_MAP"],
        &["COLOR_MAP"],
    );
}
