use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use byteorder::{LittleEndian, ReadBytesExt};

#[derive(Debug, Clone)]
pub struct BootSector {
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sector: u16,
    pub number_of_fats: u8,
    pub sectors_per_fat: u32,
    pub root_dir_cluster: u32, 
}

pub struct Fat32Image {
    file: File,
    pub boot_sector: BootSector,
}

pub struct DirectoryEntry {
    pub name: [u8; 11],     //Name
    pub attributes: u8,     //Folder or File
    pub cluster_high: u16,  //Top address of cluster
    pub cluster_low: u16,   //Bottom address of cluster
    pub size: u32,          //Size of file (bytes)
}

impl Fat32Image {
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut file = File::open(path)?;

        // 1. Byte per sector (offset 11, 2 bytes)
        file.seek(SeekFrom::Start(11))?;
        let bytes_per_sector = file.read_u16::<LittleEndian>()?;

        // 2. Sectors per cluster (offset 13, 1 byte)
        let sectors_per_cluster = file.read_u8()?;

        // 3. Reserved sectors (offset 14, 2 bytes)
        let reserved_sector = file.read_u16::<LittleEndian>()?;

        // 4. Number of FATs (offset 16, 1 byte)
        let number_of_fats = file.read_u8()?;

        // 5. Sectors per FAT (offset 36, 4 bytes)
        file.seek(SeekFrom::Start(36))?;
        let sectors_per_fat = file.read_u32::<LittleEndian>()?;

        // 6. Root directory cluster (offset 44, 4 bytes)
        file.seek(SeekFrom::Start(44))?;
        let root_dir_cluster = file.read_u32::<LittleEndian>()?;

        let boot_sector = BootSector {
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sector,
            number_of_fats,
            sectors_per_fat,
            root_dir_cluster,
        };

        Ok(Fat32Image { file, boot_sector })
    }

    pub fn offset_from_cluster(&self, cluster: u32) -> u64 {
        // 1. Calculate where start data
        let first_data_sector = self.boot_sector.reserved_sector as u64
            + (self.boot_sector.number_of_fats as u64 * self.boot_sector.sectors_per_fat as u64);
        
        // 2. Calculate how much sectors we should pass
        let cluster_offset = (cluster as u64 - 2) * self.boot_sector.sectors_per_cluster as u64;

        // 3. Add total and multiply by sector size
        let total_sectors = first_data_sector + cluster_offset;

        total_sectors * self.boot_sector.bytes_per_sector as u64
    }

    pub fn list_directory(&mut self, cluster: u32) -> io::Result<()> {
        let offset = self.offset_from_cluster(cluster);
        self.file.seek(SeekFrom::Start(offset))?;

        println!("Contents of the folder (Cluster {}) :", cluster);
        println!("-------------------------------------");

        for _ in 0..16 {
            //1. Read the name
            let mut name = [0u8; 11];
            self.file.read_exact(&mut name)?;

            if name[0] == 0 {
                break;
            }

            if name[0] == 0xE5 {
                self.file.seek(SeekFrom::Current(21))?;
                continue;
            }

            //2. Read attribute
            let attr = self.file.read_u8()?;

            self.file.seek(SeekFrom::Current(8))?;

            //3. Read High Cluster
            let cluster_high = self.file.read_u16::<LittleEndian>()?;

            self.file.seek(SeekFrom::Current(4))?;

            //4. Read Low Cluster
            let cluster_low = self.file.read_u16::<LittleEndian>()?;

            //5. Read size
            let size = self.file.read_u32::<LittleEndian>()?;

            //6. Build real name for display (byte -> string)
            let name_str = String::from_utf8_lossy(&name);

            //Combine high and low cluster
            let full_cluster = ((cluster_high as u32) << 16) | (cluster_low as u32);

            let is_dir = (attr & 0x10) != 0;
            let type_str = if is_dir { "<DIR>" } else { "   " };

            if attr != 0x0F {
                println!("{} {} (Size: {} bytes, Cluster: {})", type_str, name_str, size, full_cluster);
            }
        }
        Ok(())
    }
}

fn format_name(bytes: &[u8; 11]) -> String {
    //1. Separate name (8 bytes) and extension (3 bytes)
    let name_part = &bytes[0..8];
    let ext_part = &bytes[8..11];

    //2. Transform into String and cut empty space
    let name_str = String::from_utf8_lossy(name_part).trim().to_string();
    let ext_str = String::from_utf8_lossy(ext_part).trim().to_string();

    //3. Assemble. If no extension, return name only
    if ext_str.is_empty() {
        name_str.to_lowercase()
    }else {
        format!("{}.{}", name_str, ext_str).to_lowercase()
    }
}

fn main() -> io::Result<()> {
    let image_path = "fat32.img";

    println!("Reading FAT32 image from: {}", image_path);

match Fat32Image::new(image_path) {
        Ok(mut fs) => {
            println!("Image chargée.");
            
            let root_cluster = fs.boot_sector.root_dir_cluster;
            
            fs.list_directory(root_cluster)?;
        }
        Err(e) => eprintln!("Erreur : {}", e),
    }

    Ok(())
}