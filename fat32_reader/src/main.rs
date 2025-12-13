mod structs;
mod fat32;

use std::io::{self, Write};
use fat32::Fat32Image;

fn main() -> io::Result<()> {
    let image_path = "fat32.img"; 
    
    let mut fs = match Fat32Image::new(image_path) {
        Ok(img) => img,
        Err(e) => {
            eprintln!("Erreur critique : {}", e);
            return Ok(());
        }
    };

    println!("Bienvenue dans FAT32 Reader !");
    println!("Commandes disponibles : ls, cd, cat, exit");

    let mut current_cluster = fs.boot_sector.root_dir_cluster;

    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        let parts: Vec<&str> = input.trim().split_whitespace().collect();
        if parts.is_empty() { continue; }

        let command = parts[0];
        let argument = if parts.len() > 1 { Some(parts[1]) } else { None };

        match command {
            "ls" => {
                let target_path = argument.unwrap_or("");
                match fs.resolve_path(current_cluster, target_path) {
                    Ok((parent_cluster, target_name)) => {
                        match target_name {
                            None => { let _ = fs.list_directory(parent_cluster); },
                            Some(name) => {
                                match fs.find_sub_directory(parent_cluster, &name)? {
                                    Some(dir_cluster) => { let _ = fs.list_directory(dir_cluster); },
                                    None => println!("'{}' n'est pas un dossier.", name),
                                }
                            }
                        }
                    }
                    Err(e) => eprintln!("Erreur : {}", e),
                }
            }
            "cd" => {
                if let Some(path) = argument {
                    match fs.resolve_path(current_cluster, path) {
                        Ok((parent_cluster, target_name)) => {
                            match target_name {
                                None => { current_cluster = parent_cluster; println!("Retour racine."); },
                                Some(name) => {
                                    match fs.find_sub_directory(parent_cluster, &name)? {
                                        Some(new_cluster) => {
                                            current_cluster = new_cluster;
                                            println!("Dossier changé.");
                                        },
                                        None => println!("Introuvable."),
                                    }
                                }
                            }
                        },
                        Err(e) => eprintln!("Erreur : {}", e),
                    }
                } else { println!("Usage : cd <chemin>"); }
            }
            "cat" => {
                if let Some(path) = argument {
                    match fs.resolve_path(current_cluster, path) {
                        Ok((parent_cluster, Some(filename))) => {
                            if let Err(e) = fs.cat_file(parent_cluster, &filename) {
                                eprintln!("Erreur : {}", e);
                            }
                        }
                        Ok((_, None)) => println!("Cible invalide."),
                        Err(e) => eprintln!("Erreur : {}", e),
                    }
                } else { println!("Usage : cat <fichier>"); }
            }
            "exit" | "quit" => break,
            _ => println!("Commande inconnue."),
        }
    }
    Ok(())
}