use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};

#[derive(Debug, Clone)]
pub struct BiosParameterBlock {
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub num_fats: u8,
    pub root_entries: u16, // Important pour FAT12/16, doit être 0 pour FAT32
    pub total_sectors_16: u16,
    pub media: u8,
    pub fat_size_16: u16,
    pub sectors_per_track: u16,
    pub num_heads: u16,
    pub hidden_sectors: u32,
    pub total_sectors_32: u32,
    
    // Champs spécifiques FAT32 (à partir de l'offset 36)
    pub fat_size_32: u32,
    pub ext_flags: u16,
    pub fs_version: u16,
    pub root_cluster: u32,
    pub fs_info: u16,
    pub backup_boot_sector: u16,
}

impl BiosParameterBlock {
    pub fn new(file: &mut File) -> Result<Self, String> {
        // On se place au début du fichier
        file.seek(SeekFrom::Start(0))
            .map_err(|e| format!("Erreur seek: {}", e))?;

        let mut buf = [0u8; 512];
        file.read_exact(&mut buf)
            .map_err(|e| format!("Erreur lecture BPB: {}", e))?;

        // Vérification de la signature de fin (0xAA55)
        if buf[510] != 0x55 || buf[511] != 0xAA {
            return Err("Signature de boot invalide (pas FAT)".to_string());
        }

        // Parsing manuel (Little Endian)
        Ok(BiosParameterBlock {
            bytes_per_sector: u16::from_le_bytes([buf[11], buf[12]]),
            sectors_per_cluster: buf[13],
            reserved_sectors: u16::from_le_bytes([buf[14], buf[15]]),
            num_fats: buf[16],
            root_entries: u16::from_le_bytes([buf[17], buf[18]]),
            total_sectors_16: u16::from_le_bytes([buf[19], buf[20]]),
            media: buf[21],
            fat_size_16: u16::from_le_bytes([buf[22], buf[23]]),
            sectors_per_track: u16::from_le_bytes([buf[24], buf[25]]),
            num_heads: u16::from_le_bytes([buf[26], buf[27]]),
            hidden_sectors: u32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]),
            total_sectors_32: u32::from_le_bytes([buf[32], buf[33], buf[34], buf[35]]),
            
            // FAT32 Specific
            fat_size_32: u32::from_le_bytes([buf[36], buf[37], buf[38], buf[39]]),
            ext_flags: u16::from_le_bytes([buf[40], buf[41]]),
            fs_version: u16::from_le_bytes([buf[42], buf[43]]),
            root_cluster: u32::from_le_bytes([buf[44], buf[45], buf[46], buf[47]]),
            fs_info: u16::from_le_bytes([buf[48], buf[49]]),
            backup_boot_sector: u16::from_le_bytes([buf[50], buf[51]]),
        })
    }
}