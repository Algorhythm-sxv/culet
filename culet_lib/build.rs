use spirv_builder::{MetadataPrintout, SpirvBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    SpirvBuilder::new("../culet_shaders", "spirv-unknown-vulkan1.1")
        .capability(spirv_builder::Capability::Kernel)
        .print_metadata(MetadataPrintout::Full)
        .build()?;

    Ok(())
}
