use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use byteorder::{LittleEndian, ReadBytesExt};

use crate::structs::BootSector;

pub struct Fat32Image {
    file: File,
    pub boot_sector: BootSector,
}

fn format_name(bytes: &[u8; 11]) -> String {
    let name_part = &bytes[0..8];
    let ext_part = &bytes[8..11];
    let name_str = String::from_utf8_lossy(name_part).trim().to_string();
    let ext_str = String::from_utf8_lossy(ext_part).trim().to_string();

    if ext_str.is_empty() {
        name_str.to_lowercase()
    } else {
        format!("{}.{}", name_str, ext_str).to_lowercase()
    }
}

impl Fat32Image {
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut file = File::open(path)?;

        file.seek(SeekFrom::Start(11))?;
        let bytes_per_sector = file.read_u16::<LittleEndian>()?;
        let sectors_per_cluster = file.read_u8()?;
        let reserved_sectors = file.read_u16::<LittleEndian>()?;
        let number_of_fats = file.read_u8()?;

        file.seek(SeekFrom::Start(36))?;
        let sectors_per_fat = file.read_u32::<LittleEndian>()?;

        file.seek(SeekFrom::Start(44))?;
        let root_dir_cluster = file.read_u32::<LittleEndian>()?;

        let boot_sector = BootSector {
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sector: reserved_sectors,
            number_of_fats,
            sectors_per_fat,
            root_dir_cluster,
        };

        Ok(Fat32Image { file, boot_sector })
    }

    pub fn offset_from_cluster(&self, cluster: u32) -> u64 {
        let first_data_sector = self.boot_sector.reserved_sector as u64
            + (self.boot_sector.number_of_fats as u64 * self.boot_sector.sectors_per_fat as u64);
        
        let cluster_offset = (cluster as u64 - 2) * self.boot_sector.sectors_per_cluster as u64;
        let total_sectors = first_data_sector + cluster_offset;
        
        total_sectors * self.boot_sector.bytes_per_sector as u64
    }

    pub fn list_directory(&mut self, cluster: u32) -> io::Result<()> {
        let offset = self.offset_from_cluster(cluster);
        self.file.seek(SeekFrom::Start(offset))?;

        println!("Contenu du dossier (Cluster {}) :", cluster);
        println!("-------------------------------------");

        for _ in 0..100 {
            let mut entry_bytes = [0u8; 32];
            self.file.read_exact(&mut entry_bytes)?;

            if entry_bytes[0] == 0 { break; }
            if entry_bytes[0] == 0xE5 { continue; }
            let attr = entry_bytes[11];
            if attr == 0x0F { continue; }

            let raw_name: [u8; 11] = entry_bytes[0..11].try_into().unwrap();
            let pretty_name = format_name(&raw_name);

            let cluster_hi = u16::from_le_bytes([entry_bytes[20], entry_bytes[21]]);
            let cluster_lo = u16::from_le_bytes([entry_bytes[26], entry_bytes[27]]);
            let size = u32::from_le_bytes([entry_bytes[28], entry_bytes[29], entry_bytes[30], entry_bytes[31]]);
            
            let full_cluster = ((cluster_hi as u32) << 16) | (cluster_lo as u32);
            let is_dir = (attr & 0x10) != 0;
            let type_icon = if is_dir { "📁" } else { "📄" };

            println!("{} {:<15} (Taille: {} octets, Cluster: {})", type_icon, pretty_name, size, full_cluster);
        }
        Ok(())
    }

    pub fn cat_file(&mut self, current_cluster: u32, filename: &str) -> io::Result<()> {
        let offset = self.offset_from_cluster(current_cluster);
        self.file.seek(SeekFrom::Start(offset))?;

        for _ in 0..100 {
            let mut entry_bytes = [0u8; 32];
            self.file.read_exact(&mut entry_bytes)?;

            if entry_bytes[0] == 0 { break; }
            if entry_bytes[0] == 0xE5 { continue; }
            if entry_bytes[11] == 0x0F { continue; }

            let raw_name: [u8; 11] = entry_bytes[0..11].try_into().unwrap();
            let name = format_name(&raw_name);

            if name == filename.to_lowercase() {
                let is_dir = (entry_bytes[11] & 0x10) != 0;
                if is_dir {
                    println!("Erreur : '{}' est un dossier.", filename);
                    return Ok(());
                }

                let cluster_hi = u16::from_le_bytes([entry_bytes[20], entry_bytes[21]]);
                let cluster_lo = u16::from_le_bytes([entry_bytes[26], entry_bytes[27]]);
                let size = u32::from_le_bytes([entry_bytes[28], entry_bytes[29], entry_bytes[30], entry_bytes[31]]);
                let target_cluster = ((cluster_hi as u32) << 16) | (cluster_lo as u32);

                let data_offset = self.offset_from_cluster(target_cluster);
                self.file.seek(SeekFrom::Start(data_offset))?;

                let mut content = vec![0u8; size as usize];
                self.file.read_exact(&mut content)?;
                let text = String::from_utf8_lossy(&content);
                
                println!("Contenu de '{}':", filename);
                println!("--------------------------------------");
                println!("{}", text);
                println!("--------------------------------------");
                return Ok(());
            }
        }
        println!("Erreur : Fichier '{}' introuvable.", filename);
        Ok(())
    }

    pub fn find_sub_directory(&mut self, current_cluster: u32, dirname: &str) -> io::Result<Option<u32>> {
        let offset = self.offset_from_cluster(current_cluster);
        self.file.seek(SeekFrom::Start(offset))?;

        for _ in 0..100 {
            let mut entry_bytes = [0u8; 32];
            self.file.read_exact(&mut entry_bytes)?;

            if entry_bytes[0] == 0 { break; }
            if entry_bytes[0] == 0xE5 { continue; }
            if entry_bytes[11] == 0x0F { continue; }

            let raw_name: [u8; 11] = entry_bytes[0..11].try_into().unwrap();
            let name = format_name(&raw_name);

            if name == dirname.to_lowercase() {
                let is_dir = (entry_bytes[11] & 0x10) != 0;
                if is_dir {
                    let cluster_hi = u16::from_le_bytes([entry_bytes[20], entry_bytes[21]]);
                    let cluster_lo = u16::from_le_bytes([entry_bytes[26], entry_bytes[27]]);
                    let mut target_cluster = ((cluster_hi as u32) << 16) | (cluster_lo as u32);
                    if target_cluster == 0 { target_cluster = 2; }
                    return Ok(Some(target_cluster));
                }
                return Ok(None);
            }
        }
        Ok(None)
    }

    pub fn resolve_path(&mut self, start_cluster: u32, path: &str) -> io::Result<(u32, Option<String>)> {
        let (mut current_cluster, path_to_process) = if path.starts_with('/') {
            (self.boot_sector.root_dir_cluster, &path[1..])
        } else {
            (start_cluster, path)
        };

        let parts: Vec<&str> = path_to_process.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return Ok((current_cluster, None));
        }

        let (filename, parent_dirs) = parts.split_last().unwrap();

        for dir_name in parent_dirs {
            match self.find_sub_directory(current_cluster, dir_name)? {
                Some(next_cluster) => current_cluster = next_cluster,
                None => {
                    return Err(io::Error::new(io::ErrorKind::NotFound, format!("Dossier introuvable : {}", dir_name)));
                }
            }
        }
        Ok((current_cluster, Some(filename.to_string())))
    }
}