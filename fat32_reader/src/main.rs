use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
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

        for _ in 0..100 {
            //1. Read the name
            let mut entry_bytes = [0u8; 32];
            self.file.read_exact(&mut entry_bytes)?;

            if entry_bytes[0] == 0 {
                break;
            }

            if entry_bytes[0] == 0xE5 {
                continue;
            }

            //2. Read attribute
            let attr = entry_bytes[11];
            if attr == 0x0F {
                continue;
            }

            let raw_name: [u8; 11] = entry_bytes[0..11].try_into().unwrap();
            let pretty_name = format_name(&raw_name);

            let cluster_hi = u16::from_le_bytes([entry_bytes[20], entry_bytes[21]]);
            let cluster_lo = u16::from_le_bytes([entry_bytes[26], entry_bytes[27]]);
            let size = u32::from_le_bytes([entry_bytes[28], entry_bytes[29], entry_bytes[30], entry_bytes[31]]);
            
            let full_cluster = ((cluster_hi as u32) << 16) | (cluster_lo as u32);
            let is_dir = (attr & 0x10) != 0;
            
            let type_icon = if is_dir { "📁" } else { "📄" }; // Petites icônes sympas

            println!("{} {:<15} (Taille: {} octets, Cluster: {})", type_icon, pretty_name, size, full_cluster);
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
    
    let mut fs = match Fat32Image::new(image_path) {
        Ok(img) => img,
        Err(e) => {
            eprintln!("Critical error : {}", e);
            return Ok(());
        }
    };

    println!("Welcome in FAT32 Reader !");
    println!("Available commands : ls, exit");

    let current_cluster = fs.boot_sector.root_dir_cluster;

    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        let command = input.trim();

        match command {
            "ls" => {
                if let Err(e) = fs.list_directory(current_cluster) {
                    eprintln!("Error in listing : {}", e);
                }
            }
            "exit" | "quit" => {
                println!("Goodbye !");
                break;
            }
            "" => {}
            _ => {
                println!("Unknown command : '{}'", command);
            }
        }
    }

    Ok(())
}