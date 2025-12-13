use byteorder::{LittleEndian, ReadBytesExt};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

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
    pub name: [u8; 11],    //Name
    pub attributes: u8,    //Folder or File
    pub cluster_high: u16, //Top address of cluster
    pub cluster_low: u16,  //Bottom address of cluster
    pub size: u32,         //Size of file (bytes)
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
            let size = u32::from_le_bytes([
                entry_bytes[28],
                entry_bytes[29],
                entry_bytes[30],
                entry_bytes[31],
            ]);

            let full_cluster = ((cluster_hi as u32) << 16) | (cluster_lo as u32);
            let is_dir = (attr & 0x10) != 0;

            let type_icon = if is_dir { "📁" } else { "📄" }; // Petites icônes sympas

            println!(
                "{} {:<15} (Taille: {} octets, Cluster: {})",
                type_icon, pretty_name, size, full_cluster
            );
        }
        Ok(())
    }

    pub fn cat_file(&mut self, current_cluster: u32, filename: &str) -> io::Result<()> {
        let offset = self.offset_from_cluster(current_cluster);
        self.file.seek(SeekFrom::Start(offset))?;

        for _ in 0..100 {
            let mut entry_bytes = [0u8; 32];
            self.file.read_exact(&mut entry_bytes)?;

            if entry_bytes[0] == 0 {
                break;
            }

            if entry_bytes[0] == 0xE5 {
                continue;
            }

            if entry_bytes[11] == 0x0F {
                continue;
            }

            let raw_name: [u8; 11] = entry_bytes[0..11].try_into().unwrap();
            let name = format_name(&raw_name);

            if name == filename.to_lowercase() {
                let cluster_hi = u16::from_le_bytes([entry_bytes[20], entry_bytes[21]]);
                let cluster_lo = u16::from_le_bytes([entry_bytes[26], entry_bytes[27]]);
                let size = u32::from_le_bytes([
                    entry_bytes[28],
                    entry_bytes[29],
                    entry_bytes[30],
                    entry_bytes[31],
                ]);

                let target_cluster = ((cluster_hi as u32) << 16) | (cluster_lo as u32);

                let is_dir = (entry_bytes[11] & 0x10) != 0;
                if is_dir {
                    println!(
                        "Error: '{}' is a directory, cannot display contents.",
                        filename
                    );
                    return Ok(());
                }

                let data_offset = self.offset_from_cluster(target_cluster);
                self.file.seek(SeekFrom::Start(data_offset))?;

                let mut content = vec![0u8; size as usize];
                self.file.read_exact(&mut content)?;

                let text = String::from_utf8_lossy(&content);
                println!("Contents of file '{}':", filename);
                println!("--------------------------------------");
                println!("{}", text);
                println!("--------------------------------------");

                return Ok(());
            }
        }

        println!("File '{}' not found in current directory.", filename);
        Ok(())
    }

    pub fn find_sub_directory(
        &mut self,
        current_cluster: u32,
        dir_name: &str,
    ) -> io::Result<Option<u32>> {
        let offset = self.offset_from_cluster(current_cluster);
        self.file.seek(SeekFrom::Start(offset))?;

        for _ in 0..100 {
            let mut entry_bytes = [0u8; 32];
            self.file.read_exact(&mut entry_bytes)?;

            if entry_bytes[0] == 0 {
                break;
            }

            if entry_bytes[0] == 0xE5 {
                continue;
            }

            if entry_bytes[11] == 0x0F {
                continue;
            }

            let raw_name: [u8; 11] = entry_bytes[0..11].try_into().unwrap();
            let name = format_name(&raw_name);

            if name == dir_name.to_lowercase() {
                let is_dir = (entry_bytes[11] & 0x10) != 0;

                if is_dir {
                    let cluster_hi = u16::from_le_bytes([entry_bytes[20], entry_bytes[21]]);
                    let cluster_lo = u16::from_le_bytes([entry_bytes[26], entry_bytes[27]]);
                    let mut target_cluster = ((cluster_hi as u32) << 16) | (cluster_lo as u32);

                    if target_cluster == 0 {
                        target_cluster = 2;
                    }

                    return Ok(Some(target_cluster));
                } else {
                    println!("'{}' is not a directory.", dir_name);
                    return Ok(None);
                }
            }
        }
        Ok(None)
    }

    pub fn resolve_path(
        &mut self,
        start_cluster: u32,
        path: &str,
    ) -> io::Result<(u32, Option<String>)> {
        let (mut current_cluster, path_to_process) = if path.starts_with('/') {
            (self.boot_sector.root_dir_cluster, &path[1..])
        } else {
            (start_cluster, path)
        };

        let parts: Vec<&str> = path_to_process
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        if parts.is_empty() {
            return Ok((current_cluster, None));
        }

        let (filename, parent_dirs) = parts.split_last().unwrap();

        for dir_name in parent_dirs {
            match self.find_sub_directory(current_cluster, dir_name)? {
                Some(next_cluster) => current_cluster = next_cluster,
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("Dossier introuvable : {}", dir_name),
                    ));
                }
            }
        }

        Ok((current_cluster, Some(filename.to_string())))
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
    } else {
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

    let mut current_cluster = fs.boot_sector.root_dir_cluster;

    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let parts: Vec<&str> = input.trim().split_whitespace().collect();

        if parts.is_empty() {
            continue;
        }

        let command = parts[0];
        let argument = if parts.len() > 1 {
            Some(parts[1])
        } else {
            None
        };

        match command {
            "ls" => {
                // Par défaut, on liste le dossier courant (si pas d'argument)
                let target_path = argument.unwrap_or("");

                // On utilise notre GPS pour trouver où aller
                match fs.resolve_path(current_cluster, target_path) {
                    Ok((parent_cluster, target_name)) => match target_name {
                        None => {
                            if let Err(e) = fs.list_directory(parent_cluster) {
                                eprintln!("Erreur : {}", e);
                            }
                        }
                        Some(name) => match fs.find_sub_directory(parent_cluster, &name)? {
                            Some(dir_cluster) => {
                                if let Err(e) = fs.list_directory(dir_cluster) {
                                    eprintln!("Erreur : {}", e);
                                }
                            }
                            None => {
                                println!("'{}' n'est pas un dossier ou n'existe pas.", name);
                            }
                        },
                    },
                    Err(e) => eprintln!("Chemin invalide : {}", e),
                }
            }
            "cat" => {
                if let Some(path) = argument {
                    match fs.resolve_path(current_cluster, path) {
                        Ok((parent_cluster, Some(filename))) => {
                            if let Err(e) = fs.cat_file(parent_cluster, &filename) {
                                eprintln!("Erreur de lecture : {}", e);
                            }
                        }
                        Ok((_, None)) => {
                            println!("Veuillez spécifier un fichier (pas un dossier).")
                        }
                        Err(e) => eprintln!("Chemin invalide : {}", e),
                    }
                } else {
                    println!("Usage : cat <chemin/vers/fichier>");
                }
            }
            "cd" => {
                if let Some(path) = argument {
                    match fs.resolve_path(current_cluster, path) {
                        Ok((parent_cluster, target_name)) => {
                            match target_name {
                                None => {
                                    current_cluster = parent_cluster;
                                    println!("Retour à la racine.");
                                }
                                Some(name) => {
                                    match fs.find_sub_directory(parent_cluster, &name)? {
                                        Some(new_cluster) => {
                                            current_cluster = new_cluster;
                                            println!("Dossier changé.");
                                        }
                                        None => println!("Dossier '{}' introuvable.", name),
                                    }
                                }
                            }
                        }
                        Err(e) => eprintln!("Erreur : {}", e),
                    }
                } else {
                    println!("Usage : cd <chemin>");
                }
            }
            "exit" | "quit" => {
                println!("Au revoir !");
                break;
            }
            _ => {
                println!("Commande inconnue : '{}'", command);
            }
        }
    }

    Ok(())
}
