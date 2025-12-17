extern crate alloc;
use alloc::vec::Vec;
use alloc::string::{String, ToString};
use alloc::format;
use core::convert::TryInto;

use super::structs::BootSector;

pub struct Fat32Volume<'a> {
    data: &'a mut [u8], 
    pub boot_sector: BootSector,
    pub current_cluster: u32,
}

impl<'a> Fat32Volume<'a> {
    
    pub fn new(data: &'a mut [u8]) -> Self {
        let read_u16 = |offset| u16::from_le_bytes(data[offset..offset+2].try_into().unwrap());
        let read_u32 = |offset| u32::from_le_bytes(data[offset..offset+4].try_into().unwrap());
        let read_u8 = |offset| data[offset];

        let boot_sector = BootSector {
            bytes_per_sector: read_u16(11),
            sectors_per_cluster: read_u8(13),
            reserved_sectors: read_u16(14),
            number_of_fats: read_u8(16),
            root_entries: read_u16(17),
            total_sectors_16: read_u16(19),
            media_descriptor: read_u8(21),
            sectors_per_fat_16: read_u16(22),
            sectors_per_track: read_u16(24),
            heads: read_u16(26),
            hidden_sectors: read_u32(28),
            total_sectors_32: read_u32(32),
            sectors_per_fat_32: read_u32(36),
            ext_flags: read_u16(40),
            fs_version: read_u16(42),
            root_dir_cluster: read_u32(44),
        };

        let root = boot_sector.root_dir_cluster;
        Fat32Volume { data, boot_sector, current_cluster: root }
    }

    pub fn get_info(&self) -> String {
        // SAFETY: On copie les champs 'packed' dans des variables locales avant de les utiliser
        // pour satisfaire le compilateur Rust qui interdit les références non alignées.
        let bps = self.boot_sector.bytes_per_sector;
        let spc = self.boot_sector.sectors_per_cluster;
        let root_cluster = self.boot_sector.root_dir_cluster;

        format!(
            "Info:\n - Sector Size: {}\n - Cluster Size: {}\n - Root Cluster: {}\n - Current Cluster: {}",
            bps, 
            spc, 
            root_cluster,
            self.current_cluster
        )
    }

    fn offset_from_cluster(&self, cluster: u32) -> usize {
        let reserved = self.boot_sector.reserved_sectors as u64;
        let fats = self.boot_sector.number_of_fats as u64;
        let spf = self.boot_sector.sectors_per_fat_32 as u64;
        let spc = self.boot_sector.sectors_per_cluster as u64;
        let bps = self.boot_sector.bytes_per_sector as u64;

        let first_data_sector = reserved + (fats * spf);
        let cluster_num = if cluster < 2 { 2 } else { cluster }; 
        let cluster_offset = (cluster_num as u64 - 2) * spc;
        
        let total_sectors = first_data_sector + cluster_offset;
        (total_sectors * bps) as usize
    }

    fn allocate_cluster(&mut self) -> Option<u32> {
        let fat_start = (self.boot_sector.reserved_sectors as u64 * self.boot_sector.bytes_per_sector as u64) as usize;
        let total_clusters = (self.boot_sector.sectors_per_fat_32 * self.boot_sector.bytes_per_sector as u32) / 4;

        for i in 3..total_clusters {
            let offset = fat_start + (i as usize * 4);
            let entry = u32::from_le_bytes(self.data[offset..offset+4].try_into().unwrap());
            if entry == 0 {
                let eof: u32 = 0x0FFFFFFF;
                self.data[offset..offset+4].copy_from_slice(&eof.to_le_bytes());
                return Some(i);
            }
        }
        None
    }

    pub fn list_current(&self) -> Vec<String> {
        self.list_directory(self.current_cluster)
    }

    fn list_directory(&self, cluster: u32) -> Vec<String> {
        let start_offset = self.offset_from_cluster(cluster);
        let mut cursor = start_offset;
        let mut files = Vec::new();

        for _ in 0..128 { 
            if cursor + 32 > self.data.len() { break; }
            let entry = &self.data[cursor..cursor+32];
            
            if entry[0] == 0 { break; } 
            if entry[0] == 0xE5 { cursor += 32; continue; } 

            let attr = entry[11];
            if attr != 0x0F && (attr & 0x08) == 0 {
                let name = String::from_utf8_lossy(&entry[0..8]).trim().to_string();
                let ext = String::from_utf8_lossy(&entry[8..11]).trim().to_string();
                
                let is_dir = (attr & 0x10) != 0;
                let type_str = if is_dir { "<DIR>" } else { "     " };
                
                let full_name = if is_dir || ext.is_empty() { name } else { format!("{}.{}", name, ext) };
                let size = u32::from_le_bytes(entry[28..32].try_into().unwrap());
                
                files.push(format!("{} {} ({} bytes)", type_str, full_name, size));
            }
            cursor += 32;
        }
        files
    }

    pub fn change_directory(&mut self, dirname: &str) -> Result<(), &'static str> {
        let start_offset = self.offset_from_cluster(self.current_cluster);
        let mut cursor = start_offset;

        if dirname == "." { return Ok(()); }

        for _ in 0..128 {
            if cursor + 32 > self.data.len() { break; }
            let entry = &self.data[cursor..cursor+32];
            if entry[0] == 0 { break; } 

            let name = String::from_utf8_lossy(&entry[0..8]).trim().to_string();
            let ext = String::from_utf8_lossy(&entry[8..11]).trim().to_string();
            let mut full_name = name.clone();
            if !ext.is_empty() { full_name = format!("{}.{}", name, ext); }

            if name.eq_ignore_ascii_case(dirname) || full_name.eq_ignore_ascii_case(dirname) {
                let attr = entry[11];
                if (attr & 0x10) != 0 { 
                    let cluster_hi = u16::from_le_bytes(entry[20..22].try_into().unwrap());
                    let cluster_lo = u16::from_le_bytes(entry[26..28].try_into().unwrap());
                    let mut cluster = ((cluster_hi as u32) << 16) | (cluster_lo as u32);
                    
                    if cluster == 0 { cluster = self.boot_sector.root_dir_cluster; }
                    
                    self.current_cluster = cluster;
                    return Ok(());
                } else {
                    return Err("Ce n'est pas un dossier");
                }
            }
            cursor += 32;
        }
        Err("Dossier introuvable")
    }

    pub fn read_file(&self, filename: &str) -> Result<Vec<u8>, &'static str> {
        let start_offset = self.offset_from_cluster(self.current_cluster);
        let mut cursor = start_offset;

        for _ in 0..128 {
            if cursor + 32 > self.data.len() { break; }
            let entry = &self.data[cursor..cursor+32];
            if entry[0] == 0 { break; } 
            
            let name = String::from_utf8_lossy(&entry[0..8]).trim().to_string();
            let ext = String::from_utf8_lossy(&entry[8..11]).trim().to_string();
            let full_name = if ext.is_empty() { name.clone() } else { format!("{}.{}", name, ext) };

            if full_name.eq_ignore_ascii_case(filename) {
                let attr = entry[11];
                if (attr & 0x10) != 0 { return Err("C'est un dossier, utilisez cd"); }

                let cluster_hi = u16::from_le_bytes(entry[20..22].try_into().unwrap());
                let cluster_lo = u16::from_le_bytes(entry[26..28].try_into().unwrap());
                let cluster = ((cluster_hi as u32) << 16) | (cluster_lo as u32);
                let size = u32::from_le_bytes(entry[28..32].try_into().unwrap());

                let data_offset = self.offset_from_cluster(cluster);
                if data_offset + size as usize <= self.data.len() {
                    let mut content = Vec::new();
                    content.extend_from_slice(&self.data[data_offset..data_offset + size as usize]);
                    return Ok(content);
                }
            }
            cursor += 32;
        }
        Err("Fichier introuvable")
    }

    pub fn create_file(&mut self, filename: &str, content: &[u8]) -> Result<(), &'static str> {
        let free_cluster = self.allocate_cluster().ok_or("Disque plein")?;
        let data_offset = self.offset_from_cluster(free_cluster);
        
        self.data[data_offset..data_offset + content.len()].copy_from_slice(content);

        let dir_offset = self.offset_from_cluster(self.current_cluster);
        self.write_dir_entry(dir_offset, filename, free_cluster, content.len() as u32)
    }

    fn write_dir_entry(&mut self, dir_offset: usize, filename: &str, cluster: u32, size: u32) -> Result<(), &'static str> {
        let mut cursor = dir_offset;
        for _ in 0..64 {
            let marker = self.data[cursor];
            if marker == 0x00 || marker == 0xE5 {
                let parts: Vec<&str> = filename.split('.').collect();
                let name = parts.get(0).unwrap_or(&"UNKNOWN");
                let ext = parts.get(1).unwrap_or(&"   ");
                
                let mut name_field = [0x20u8; 11]; 
                for (i, b) in name.as_bytes().iter().take(8).enumerate() { name_field[i] = b.to_ascii_uppercase(); }
                for (i, b) in ext.as_bytes().iter().take(3).enumerate() { name_field[8 + i] = b.to_ascii_uppercase(); }

                self.data[cursor..cursor+11].copy_from_slice(&name_field);
                self.data[cursor+11] = 0x20; 
                let high = ((cluster >> 16) as u16).to_le_bytes();
                self.data[cursor+20] = high[0]; self.data[cursor+21] = high[1];
                let low = (cluster as u16).to_le_bytes();
                self.data[cursor+26] = low[0]; self.data[cursor+27] = low[1];
                self.data[cursor+28..cursor+32].copy_from_slice(&size.to_le_bytes());
                return Ok(());
            }
            cursor += 32;
        }
        Err("Répertoire plein")
    }
}

// ----------------------------------------------------------------
// TESTS (Mandatory)
// ----------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn create_mock_volume() -> Vec<u8> {
        let mut data = vec![0u8; 1024 * 1024]; 
        
        data[11] = 0x00; data[12] = 0x02; // 512 bytes per sector
        data[13] = 1;                     // 1 sector per cluster
        data[14] = 32; data[15] = 0;      // 32 reserved
        data[16] = 2;                     // 2 FATs
        data[36] = 100; data[37] = 0; data[38] = 0; data[39] = 0; // 100 sectors per FAT
        data[44] = 2; data[45] = 0; data[46] = 0; data[47] = 0;   // Root at 2

        data
    }

    #[test]
    fn test_volume_initialization() {
        let mut data = create_mock_volume();
        let volume = Fat32Volume::new(&mut data);
        
        let bps = volume.boot_sector.bytes_per_sector;
        let root = volume.boot_sector.root_dir_cluster;

        assert_eq!(bps, 512);
        assert_eq!(root, 2);
        assert_eq!(volume.current_cluster, 2);
    }

    #[test]
    fn test_offset_calculation() {
        let mut data = create_mock_volume();
        let volume = Fat32Volume::new(&mut data);
        let offset = volume.offset_from_cluster(2);
        assert_eq!(offset, 118784);
    }
}