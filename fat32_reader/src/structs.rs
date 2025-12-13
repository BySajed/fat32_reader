#[derive(Debug, Clone, Copy)]
pub struct BootSector {
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sector: u16,
    pub number_of_fats: u8,
    pub sectors_per_fat: u32,
    pub root_dir_cluster: u32,
}

pub struct DirectoryEntry {
    pub name: [u8; 11],
    pub attributes: u8,
    pub cluster_high: u16,
    pub cluster_low: u16,
    pub size: u32,
}